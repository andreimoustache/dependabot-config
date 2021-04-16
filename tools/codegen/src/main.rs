#![warn(rust_2018_idioms, single_use_lifetimes)]

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use anyhow::Result;
use quote::{format_ident, quote};
use syn::visit_mut::{self, VisitMut};
use walkdir::WalkDir;

fn main() -> Result<()> {
    gen_from_str()?;
    gen_assert_impl()?;
    Ok(())
}

fn root_dir() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop(); // codegen
    dir.pop(); // tools
    dir
}

fn gen_from_str() -> Result<()> {
    let root_dir = &root_dir();

    let mut out = String::new();
    out += &format!("// This file is @generated by {}.\n", env!("CARGO_BIN_NAME"));
    out += "// It is not intended for manual editing.\n";
    out += "\n";

    let mut tokens = quote! {
        use std::str::FromStr;
        use crate::*;
    };

    for &f in &["src/lib.rs", "src/v1.rs", "src/v2.rs"] {
        let s = fs::read_to_string(root_dir.join(f))?;
        let ast = syn::parse_file(&s)?;

        let module = if f.ends_with("lib.rs") {
            quote! {}
        } else {
            let name = format_ident!("{}", Path::new(f).file_stem().unwrap().to_string_lossy());
            quote! { #name:: }
        };

        for i in ast.items {
            match i {
                syn::Item::Struct(syn::ItemStruct { vis, ident, .. })
                | syn::Item::Enum(syn::ItemEnum { vis, ident, .. })
                    if matches!(vis, syn::Visibility::Public(..)) =>
                {
                    tokens.extend(quote! {
                        impl FromStr for #module#ident {
                            type Err = Error;
                            fn from_str(s: &str) -> Result<Self, Self::Err> {
                                serde_yaml::from_str(s).map_err(Error::new)
                            }
                        }
                    });
                }
                _ => {}
            }
        }
    }

    out += &tokens.to_string();

    fs::write(root_dir.join("src/gen/from_str.rs"), out)?;

    Ok(())
}

fn gen_assert_impl() -> Result<()> {
    let root_dir = &root_dir();

    let mut out = String::new();
    out += &format!("// This file is @generated by {}.\n", env!("CARGO_BIN_NAME"));
    out += "// It is not intended for manual editing.\n";
    out += "\n";

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

        let mut visitor = ItemVisitor::new(module, |item, module| match item {
            syn::Item::Struct(syn::ItemStruct { vis, ident, generics, .. })
            | syn::Item::Enum(syn::ItemEnum { vis, ident, generics, .. })
            | syn::Item::Union(syn::ItemUnion { vis, ident, generics, .. })
            | syn::Item::Type(syn::ItemType { vis, ident, generics, .. })
                if matches!(vis, syn::Visibility::Public(..)) =>
            {
                let lt_count = generics.lifetimes().count();
                let lt = if lt_count > 0 {
                    let lt = (0..lt_count).map(|_| quote! { '_ });
                    quote! { <#(#lt)*> }
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
        });
        visitor.visit_file_mut(&mut ast);
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

    fs::write(root_dir.join("src/gen/assert_impl.rs"), out)?;

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
