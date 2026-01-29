use std::{
    collections::{BTreeMap, HashSet},
    convert::Infallible,
    env::VarError,
    fs::File,
    io::BufReader,
    path::PathBuf,
};

use capslock::Capability;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{
    Ident, LitStr, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};
use thiserror::Error;

#[derive(Debug)]
struct Func {
    name: Ident,
    path: PathBuf,
    path_span: Span,
}

impl Func {
    fn generate(self) -> syn::Result<TokenStream> {
        let mut caps = CapSet::default();

        let path = if self.path.is_absolute() {
            self.path.clone()
        } else {
            let manifest =
                std::env::var("CARGO_MANIFEST_DIR").map_err(|e| Error::ManifestPath {
                    e,
                    span: self.path_span,
                })?;
            PathBuf::from(manifest).join(&self.path)
        };

        let input = BufReader::new(File::open(path).map_err(|e| Error::OpenInput {
            e,
            path: self.path.clone(),
            span: self.path_span,
        })?);

        for (name, local) in cm::Document::<String, Capability>::from_reader(input)
            .map_err(|e| Error::Cm {
                e,
                span: self.path_span,
            })?
            .into_iter()
        {
            caps.insert(name, local.into_iter());
        }

        let name = self.name;

        Ok(quote! {
            pub fn #name(syscall: &str) -> Option<impl Iterator<Item = ::capslock::Capability> + 'static> {
                use ::std::collections::{BTreeMap, HashSet};
                use ::std::str::FromStr;
                use ::std::string::String;
                use ::std::sync::LazyLock;

                use ::capslock::Capability;

                static CAPS: LazyLock<BTreeMap<String, HashSet<Capability>>> = LazyLock::new(|| {
                    #caps
                });

                CAPS.get(syscall).map(|set| {
                    set.iter().copied()
                })
            }
        })
    }
}

impl Parse for Func {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![,]>()?;
        let path: LitStr = input.parse()?;
        let path_span = path.span();

        Ok(Self {
            name,
            path: PathBuf::from(path.value()),
            path_span,
        })
    }
}

#[proc_macro]
pub fn parse(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let func = parse_macro_input!(input as Func);

    match func.generate() {
        Ok(tokens) => tokens,
        Err(e) => e.into_compile_error(),
    }
    .into()
}

#[derive(Error, Debug)]
enum Error {
    #[error("cm parsing error: {e}")]
    Cm {
        #[source]
        e: cm::Error<Infallible, capslock::ParseError>,
        span: Span,
    },

    #[error("getting crate manifest path: {e:?}")]
    ManifestPath {
        #[source]
        e: VarError,
        span: Span,
    },

    #[error("opening capslock input at {path:?}: {e}")]
    OpenInput {
        #[source]
        e: std::io::Error,
        path: PathBuf,
        span: Span,
    },
}

impl Error {
    fn span(&self) -> Span {
        match self {
            Error::Cm { span, .. } => *span,
            Error::ManifestPath { span, .. } => *span,
            Error::OpenInput { span, .. } => *span,
        }
    }
}

impl From<Error> for syn::Error {
    fn from(e: Error) -> Self {
        syn::Error::new(e.span(), e)
    }
}

#[derive(Default)]
struct CapSet(BTreeMap<String, HashSet<Capability>>);

impl CapSet {
    pub fn insert(&mut self, name: impl ToString, caps: impl Iterator<Item = Capability>) {
        self.0.insert(name.to_string(), caps.collect());
    }
}

impl ToTokens for CapSet {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let caps = self.0.iter().map(|(name, caps)| Cap {
            name: name.as_str(),
            caps,
        });

        tokens.extend(quote! {
            BTreeMap::from([
                #( #caps ),*
            ])
        });
    }
}

struct Cap<'a> {
    name: &'a str,
    caps: &'a HashSet<Capability>,
}

impl<'a> ToTokens for Cap<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self { name, caps } = self;
        let caps = caps.iter().map(|cap| {
            let name: &'static str = cap.into();

            quote! {
                // I don't love this unwrap() in the generated code, but here we are.
                ::capslock::Capability::from_str(#name).unwrap()
            }
        });

        tokens.extend(quote! {
            (
                String::from(#name),
                HashSet::from([
                    #( #caps ),*
                ]),
            )
        })
    }
}
