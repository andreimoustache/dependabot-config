#![warn(rust_2018_idioms, single_use_lifetimes)]

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use anyhow::Result;
use heck::{KebabCase, SnakeCase};
use quote::{format_ident, quote};
use syn::visit_mut::{self, VisitMut};
use walkdir::WalkDir;

fn main() -> Result<()> {
    gen_from_str()?;
    gen_display()?;
    gen_assert_impl()?;
    Ok(())
}

fn root_dir() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop(); // codegen
    dir.pop(); // tools
    dir
}

fn header() -> String {
    concat!(
        "// This file is @generated by ",
        env!("CARGO_BIN_NAME"),
        ".\n",
        "// It is not intended for manual editing.\n",
        "\n"
    )
    .into()
}

fn gen_from_str() -> Result<()> {
    let root_dir = &root_dir();

    let mut out = header();

    let mut tokens = quote! {
        use std::str::FromStr;
        use crate::*;
    };

    let files = &["src/lib.rs", "src/v1.rs", "src/v2.rs"];

    for &f in files {
        let s = fs::read_to_string(root_dir.join(f))?;
        let mut ast = syn::parse_file(&s)?;

        let module = if f.ends_with("lib.rs") {
            vec![]
        } else {
            let name = format_ident!("{}", Path::new(f).file_stem().unwrap().to_string_lossy());
            vec![name.into()]
        };

        ItemVisitor::new(module, |item, module| match item {
            syn::Item::Struct(syn::ItemStruct { vis, ident, .. })
            | syn::Item::Enum(syn::ItemEnum { vis, ident, .. })
                if matches!(vis, syn::Visibility::Public(..)) =>
            {
                tokens.extend(quote! {
                    impl FromStr for #(#module::)* #ident {
                        type Err = Error;
                        fn from_str(s: &str) -> Result<Self, Self::Err> {
                            serde_yaml::from_str(s).map_err(Error::new)
                        }
                    }
                });
            }
            _ => {}
        })
        .visit_file_mut(&mut ast);
    }

    out += &tokens.to_string();

    fs::write(root_dir.join("src/gen/from_str.rs"), out)?;

    Ok(())
}

fn serde_attr(attrs: &[syn::Attribute], name: &str) -> Option<String> {
    for meta in attrs
        .iter()
        .filter(|attr| attr.path.is_ident("serde"))
        .filter_map(|attr| attr.parse_meta().ok())
    {
        if let syn::Meta::List(list) = meta {
            for repr in list.nested.into_iter() {
                if let syn::NestedMeta::Meta(syn::Meta::NameValue(nv)) = repr {
                    if nv.path.is_ident(name) {
                        if let syn::Lit::Str(s) = nv.lit {
                            return Some(s.value());
                        }
                    }
                }
            }
        }
    }
    None
}

fn change_case(case: Option<&str>, value: String) -> String {
    match case {
        None => value,
        Some("kebab-case") => value.to_kebab_case(),
        Some("snake_case") => value.to_snake_case(),
        Some(case) => panic!("unknown case: {}", case),
    }
}

fn gen_display() -> Result<()> {
    let root_dir = &root_dir();

    let mut out = header();

    let mut tokens = quote! {
        use std::fmt;
        use crate::*;
    };

    let files = &["src/v1.rs", "src/v2.rs"];

    for &f in files {
        let s = fs::read_to_string(root_dir.join(f))?;
        let mut ast = syn::parse_file(&s)?;

        let module = {
            let name = format_ident!("{}", Path::new(f).file_stem().unwrap().to_string_lossy());
            vec![name.into()]
        };

        ItemVisitor::new(module, |item, module| match item {
            syn::Item::Enum(syn::ItemEnum { attrs, vis, ident, variants, .. })
                if matches!(vis, syn::Visibility::Public(..))
                    && variants.iter().all(|v| matches!(v.fields, syn::Fields::Unit)) =>
            {
                let case = serde_attr(attrs, "rename_all");
                let arms = variants.iter().map(|syn::Variant { attrs, ident, .. }| {
                    let rename = serde_attr(attrs, "rename");
                    let s = if let Some(rename) = rename {
                        rename
                    } else {
                        change_case(case.as_deref(), ident.to_string())
                    };
                    quote! {
                        Self::#ident => f.write_str(#s),
                    }
                });
                tokens.extend(quote! {
                    impl fmt::Display for #(#module::)* #ident {
                        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                            match self {
                                #(#arms)*
                            }
                        }
                    }
                });
            }
            _ => {}
        })
        .visit_file_mut(&mut ast);
    }

    out += &tokens.to_string();

    fs::write(root_dir.join("src/gen/display.rs"), out)?;

    Ok(())
}

fn gen_assert_impl() -> Result<()> {
    let root_dir = &root_dir();
    let out_dir = &root_dir.join("src/gen");

    let mut out = header();

    let files: BTreeSet<String> = WalkDir::new(root_dir.join("src"))
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|e| {
            let path = e.path();
            if !path.is_file() || path.extension() != Some("rs".as_ref()) {
                return None;
            }
            // Assertions are only needed for the library's public APIs.
            if path.ends_with("main.rs") {
                return None;
            }
            Some(path.to_string_lossy().into_owned())
        })
        .collect();

    let mut tokens = quote! {};
    for f in &files {
        let s = fs::read_to_string(f)?;
        let mut ast = syn::parse_file(&s)?;

        let module = if f.ends_with("lib.rs") {
            vec![]
        } else {
            let name = format_ident!("{}", Path::new(f).file_stem().unwrap().to_string_lossy());
            vec![name.into()]
        };

        ItemVisitor::new(module, |item, module| match item {
            syn::Item::Struct(syn::ItemStruct { vis, ident, generics, .. })
            | syn::Item::Enum(syn::ItemEnum { vis, ident, generics, .. })
            | syn::Item::Union(syn::ItemUnion { vis, ident, generics, .. })
            | syn::Item::Type(syn::ItemType { vis, ident, generics, .. })
                if matches!(vis, syn::Visibility::Public(..)) =>
            {
                let lt_count = generics.lifetimes().count();
                let lt = if lt_count > 0 {
                    let lt = (0..lt_count).map(|_| quote! { '_ });
                    quote! { <#(#lt),*> }
                } else {
                    quote! {}
                };
                tokens.extend(quote! {
                    assert_send::<#(#module::)* #ident #lt>();
                    assert_sync::<#(#module::)* #ident #lt>();
                    assert_unpin::<#(#module::)* #ident #lt>();
                });
            }
            _ => {}
        })
        .visit_file_mut(&mut ast);
    }

    out += &quote! {
        use crate::*;
        const _: fn() = || {
            fn assert_send<T: ?Sized + Send>() {}
            fn assert_sync<T: ?Sized + Sync>() {}
            fn assert_unpin<T: ?Sized + Unpin>() {}
            #tokens
        };
    }
    .to_string();

    fs::create_dir_all(out_dir)?;
    fs::write(out_dir.join("assert_impl.rs"), out)?;

    Ok(())
}

struct ItemVisitor<F> {
    module: Vec<syn::PathSegment>,
    f: F,
}

impl<F> ItemVisitor<F>
where
    F: FnMut(&mut syn::Item, &[syn::PathSegment]),
{
    fn new(module: Vec<syn::PathSegment>, f: F) -> Self {
        Self { module, f }
    }
}

impl<F> VisitMut for ItemVisitor<F>
where
    F: FnMut(&mut syn::Item, &[syn::PathSegment]),
{
    fn visit_item_mut(&mut self, item: &mut syn::Item) {
        if let syn::Item::Mod(item) = item {
            self.module.push(item.ident.clone().into());
            visit_mut::visit_item_mod_mut(self, item);
            self.module.pop();
            return;
        }
        (self.f)(item, &self.module);
        visit_mut::visit_item_mut(self, item);
    }
}
