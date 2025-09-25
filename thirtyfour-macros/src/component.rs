extern crate proc_macro;

use crate::bail;
use proc_macro2::TokenStream;
use proc_macro2::{Literal, Span};
use quote::{format_ident, quote, ToTokens, TokenStreamExt};
use std::collections::HashSet;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Data, Expr, ExprLit, Fields, GenericArgument, Lit, Meta, MetaNameValue, PathArguments,
    PathSegment, Token,
};

pub fn expand_component_derive(ast: syn::DeriveInput) -> TokenStream {
    ParsedOptions::try_from(ast)
        .and_then(ComponentArgs::try_from)
        .as_ref()
        .map(ComponentArgs::to_token_stream)
        .unwrap_or_else(syn::Error::to_compile_error)
}

struct ParsedOptions {
    ident: syn::Ident,
    fields: Vec<ParsedField>,
}

impl TryFrom<syn::DeriveInput> for ParsedOptions {
    type Error = syn::Error;

    fn try_from(input: syn::DeriveInput) -> Result<Self, Self::Error> {
        let fields = match input.data {
            Data::Struct(s) => match s.fields {
                Fields::Named(nf) => nf
                    .named
                    .into_iter()
                    .map(|x| ParsedField {
                        ident: x.ident.expect("Tuple or unit structs not supported"),
                        ty: x.ty,
                        attrs: x.attrs,
                    })
                    .collect(),
                _ => panic!("Tuple or unit structs not supported"),
            },
            Data::Enum(_) | Data::Union(_) => {
                panic!("Component attribute does not support enums or unions")
            }
        };

        Ok(ParsedOptions {
            ident: input.ident,
            fields,
        })
    }
}

/// The args from which we will generate the Component code.
struct ComponentArgs {
    ident: syn::Ident,
    base_ident: syn::Ident,
    fields: Vec<TokenStream>,
    field_initialisers: Vec<TokenStream>,
}

impl TryFrom<ParsedOptions> for ComponentArgs {
    type Error = syn::Error;

    fn try_from(opts: ParsedOptions) -> Result<Self, Self::Error> {
        let ident = opts.ident;
        let mut base_ident = None;
        let mut fields = Vec::with_capacity(opts.fields.len());
        let mut field_initialisers = Vec::with_capacity(opts.fields.len());

        for field in opts.fields {
            if field.is_base() {
                if base_ident.is_some() {
                    bail!(field.ident.span(), "cannot specify multiple base fields");
                }
                match &field.ty {
                    syn::Type::Path(p) => {
                        if !is_type(&p.path, &["WebElement", "thirtyfour|WebElement"]) {
                            bail!(
                                field.ty.span(),
                                "base element field must be of type thirtyfour::WebElement"
                            )
                        }
                    }
                    _ => {
                        bail!(field.ty.span(), "base element field is not a thirtyfour::WebElement")
                    }
                }
                base_ident = Some(field.ident.clone());
                continue;
            }
            let field_def = field.get_def();
            let initialiser = field.get_initialiser()?;
            fields.push(field_def);
            field_initialisers.push(initialiser);
        }

        let base_ident = match base_ident {
            Some(x) => x,
            None => {
                bail!(
                    ident.span(),
                    "base field not found. Add the #[base] attribute for the base WebElement field"
                )
            }
        };

        Ok(ComponentArgs {
            ident,
            base_ident,
            fields,
            field_initialisers,
        })
    }
}

impl ToTokens for ComponentArgs {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        // Generate impl.
        let ident = &self.ident;
        let base_ident = &self.base_ident;
        let fields = &self.fields;
        let field_initialisers = &self.field_initialisers;

        tokens.append_all(quote!(
            impl #ident {
                pub fn new(base: ::thirtyfour::WebElement) -> Self {
                    #(#field_initialisers)*
                    Self {
                        #base_ident: base,
                        #(#fields,)*
                    }
                }
            }
        ));

        // impl From<WebElement>
        tokens.append_all(quote!(
            #[automatically_derived]
            impl From<::thirtyfour::WebElement> for #ident {
                fn from(elem: ::thirtyfour::WebElement) -> Self {
                    Self::new(elem)
                }
            }
        ));

        // impl Component
        tokens.append_all(quote!(
            #[automatically_derived]
            impl ::thirtyfour::components::Component for #ident {
                fn base_element(&self) -> ::thirtyfour::WebElement {
                    self.#base_ident.clone()
                }
            }
        ));
    }
}

struct ParsedField {
    ident: syn::Ident,
    ty: syn::Type,
    attrs: Vec<syn::Attribute>,
}

impl ParsedField {
    /// True if this is the base element field.
    pub fn is_base(&self) -> bool {
        self.attrs.iter().any(|x| x.path().is_ident("base")) || self.ident == "base"
    }

    fn cfg_attr(&self) -> Option<&syn::Attribute> {
        self.attrs.iter().find(|x| x.path().is_ident("cfg"))
    }

    fn by_attr(&self) -> Option<&syn::Attribute> {
        self.attrs.iter().find(|x| x.path().is_ident("by"))
    }

    /// Get the definition for this field that should go in new().
    ///
    /// ```ignore
    /// Self {
    ///     some_field, // <-- this (including any attributes as necessary)
    /// }
    /// ```
    pub fn get_def(&self) -> TokenStream {
        let cfg_attr = self.cfg_attr();
        let ident = &self.ident;
        quote! {
            #cfg_attr
            #ident
        }
    }

    /// Get the initialiser for this field that should go in new().
    ///
    /// ```ignore
    /// let some_field = ...; // <-- this (including any attributes as necessary)
    /// Self {
    ///     some_field,
    /// }
    pub fn get_initialiser(&self) -> syn::Result<TokenStream> {
        let cfg_attr = self.cfg_attr();
        let ident = &self.ident;

        match (&self.ty, self.by_attr()) {
            (syn::Type::Path(p), Some(by_attr)) => {
                let by_tokens = ByTokens::try_from(by_attr)?;
                let ty = fix_type(p.path.clone());

                // Use type or attribute to infer single/multi resolver.
                if by_tokens.is_multi() || is_multi_resolver(&p.path) {
                    let multi_args = MultiResolverArgs::try_new(ty, by_tokens)?;

                    Ok(quote!(
                        #cfg_attr
                        let #ident = {
                            #multi_args
                        };
                    ))
                } else {
                    let single_args = SingleResolverArgs::try_new(ty, by_tokens)?;

                    Ok(quote!(
                        #cfg_attr
                        let #ident = {
                            #single_args
                        };
                    ))
                }
            }
            _ => Ok(quote!(
                #cfg_attr
                let #ident = Default::default();
            )),
        }
    }
}

#[derive(Clone)]
struct WaitOptions {
    timeout_ms: Expr,
    interval_ms: Expr,
}

impl ToTokens for WaitOptions {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let timeout_ms = &self.timeout_ms;
        let interval_ms = &self.interval_ms;
        tokens.append_all(quote!(
            Some(::thirtyfour::extensions::query::ElementQueryWaitOptions::Wait {
                timeout: ::std::time::Duration::from_millis(#timeout_ms),
                interval: ::std::time::Duration::from_millis(#interval_ms)
            })
        ));
    }
}

/// These are all the supported tokens in a `#[by(..)]` attribute.
enum ByToken {
    Id(Literal),
    Tag(Literal),
    LinkText(Literal),
    Css(Literal),
    XPath(Literal),
    Name(Literal),
    ClassName(Literal),
    Testid(Literal),
    Multi,
    /// NotEmpty is the default but can be specified to be explicit.
    NotEmpty,
    AllowEmpty,
    /// Single is the default but can be specified to be explicit.
    Single,
    First,
    IgnoreErrors,
    Description(Literal),
    Wait(WaitOptions),
    NoWait,
    CustomFn(Expr),
}

impl ByToken {
    /// Helper for making sure the right things are mutually exclusive.
    ///
    /// This is how we catch repeated options.
    fn get_unique_type(&self) -> &str {
        match &self {
            ByToken::Id(_)
            | ByToken::Tag(_)
            | ByToken::LinkText(_)
            | ByToken::Css(_)
            | ByToken::XPath(_)
            | ByToken::Name(_)
            | ByToken::ClassName(_)
            | ByToken::Testid(_) => "selector",
            ByToken::Multi => "multi",
            ByToken::NotEmpty => "not_empty",
            ByToken::AllowEmpty => "allow_empty",
            ByToken::Single => "single",
            ByToken::First => "first",
            ByToken::IgnoreErrors => "ignore_errors",
            ByToken::Description(_) => "description",
            ByToken::Wait(_) => "wait",
            ByToken::NoWait => "nowait",
            ByToken::CustomFn(_) => "custom",
        }
    }

    /// Get the above unique names that are not supported with each token.
    ///
    /// This is how mutually exclusive options are determined.
    fn get_disallowed_types(&self) -> Vec<&str> {
        match &self {
            ByToken::Single => vec!["multi", "custom", "first"],
            ByToken::First => vec!["multi", "custom", "single"],
            ByToken::NotEmpty => vec!["custom", "single", "first", "allow_empty"],
            ByToken::AllowEmpty => vec!["custom", "single", "first", "not_empty"],
            ByToken::IgnoreErrors | ByToken::Description(_) => vec!["custom"],
            ByToken::Wait(_) => vec!["custom", "nowait"],
            ByToken::NoWait => vec!["custom", "wait"],
            ByToken::CustomFn(_) => {
                vec![
                    "multi",
                    "all",
                    "single",
                    "first",
                    "ignore_errors",
                    "description",
                    "wait",
                    "nowait",
                    "allow_empty",
                ]
            }
            _ => vec![],
        }
    }
}

/// Convert `Meta` into `ByToken`.
///
/// This is where all tokens are parsed into `ByToken` variants.
impl TryFrom<Meta> for ByToken {
    type Error = syn::Error;

    fn try_from(value: Meta) -> Result<Self, Self::Error> {
        match value {
            Meta::Path(p) => match p {
                k if k.is_ident("multi") => Ok(ByToken::Multi),
                k if k.is_ident("not_empty") => Ok(ByToken::NotEmpty),
                k if k.is_ident("allow_empty") => Ok(ByToken::AllowEmpty),
                k if k.is_ident("single") => Ok(ByToken::Single),
                k if k.is_ident("first") => Ok(ByToken::First),
                k if k.is_ident("ignore_errors") => Ok(ByToken::IgnoreErrors),
                k if k.is_ident("nowait") => Ok(ByToken::NoWait),
                e => Err(syn::Error::new(
                    e.span(),
                    format!("unknown attribute {}", e.to_token_stream()),
                )),
            },
            Meta::List(list) => {
                match list.path {
                    // wait(timeout_ms = u32, interval_ms = u32)
                    ref p if p.is_ident("wait") => {
                        let mut timeout: Option<Expr> = None;
                        let mut interval: Option<Expr> = None;

                        list.parse_nested_meta(|nested| {
                            let value = || nested.value()?.parse::<Expr>();
                            match &nested.path {
                                k if k.is_ident("timeout_ms") => {
                                    if timeout.is_some() {
                                        return Err(nested.error("cannot specify timeout twice"));
                                    }
                                    timeout = Some(value()?);
                                    Ok(())
                                }
                                k if k.is_ident("interval_ms") => {
                                    if interval.is_some() {
                                        return Err(nested.error("cannot specify interval twice"));
                                    }
                                    interval = Some(value()?);
                                    Ok(())
                                }
                                e => Err(nested.error(format_args!(
                                    "unknown attribute {} (must be timeout_ms or interval_ms)",
                                    e.to_token_stream()
                                ))),
                            }
                        })?;

                        match (timeout, interval) {
                        (Some(t), Some(i)) => Ok(ByToken::Wait(WaitOptions {
                            timeout_ms: t,
                            interval_ms: i,
                        })),
                        _ => Err(syn::Error::new(list.tokens.span(), "wait attribute requires the following args: timeout_ms, interval_ms"))
                    }
                    }
                    e => Err(syn::Error::new(
                        e.span(),
                        format_args!("unknown attribute: {}", e.to_token_stream()),
                    )),
                }
            }
            Meta::NameValue(MetaNameValue {
                path,
                value,
                ..
            }) => match (path, value) {
                (
                    k,
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(v),
                        ..
                    }),
                ) if k.is_ident("id") => Ok(ByToken::Id(v.token())),
                (
                    k,
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(v),
                        ..
                    }),
                ) if k.is_ident("tag") => Ok(ByToken::Tag(v.token())),
                (
                    k,
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(v),
                        ..
                    }),
                ) if k.is_ident("link") => Ok(ByToken::LinkText(v.token())),
                (
                    k,
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(v),
                        ..
                    }),
                ) if k.is_ident("css") => Ok(ByToken::Css(v.token())),
                (
                    k,
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(v),
                        ..
                    }),
                ) if k.is_ident("xpath") => Ok(ByToken::XPath(v.token())),
                (
                    k,
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(v),
                        ..
                    }),
                ) if k.is_ident("name") => Ok(ByToken::Name(v.token())),
                (
                    k,
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(v),
                        ..
                    }),
                ) if k.is_ident("class") => Ok(ByToken::ClassName(v.token())),
                (
                    k,
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(v),
                        ..
                    }),
                ) if k.is_ident("testid") => Ok(ByToken::Testid(v.token())),
                (
                    k,
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(v),
                        ..
                    }),
                ) if k.is_ident("description") => Ok(ByToken::Description(v.token())),
                (k, expr) if k.is_ident("custom") => Ok(ByToken::CustomFn(expr)),
                (k, ..) => Err(syn::Error::new(
                    k.span(),
                    format_args!("unknown attribute: {}", k.to_token_stream()),
                )),
            },
        }
    }
}

/// Wrapper for a list of tokens so we can add methods to it.
struct ByTokens {
    tokens: Vec<ByToken>,
}

impl ByTokens {
    /// Apply validation rules to the list of tokens.
    ///
    /// This is where we determine whether these tokens are compatible with each other.
    pub fn validate(self, span: Span) -> syn::Result<Self> {
        let mut unique_tokens = HashSet::new();

        for token in self.tokens.iter() {
            let t = token.get_unique_type();
            if !unique_tokens.insert(t) {
                bail!(span, "duplicate token '{t}' (cannot specify multiple)")
            }
        }
        for token in self.tokens.iter() {
            let disallowed = token.get_disallowed_types();
            for t in disallowed {
                if unique_tokens.contains(t) {
                    let unique = token.get_unique_type();
                    bail!(span, "cannot specify '{unique}' with '{t}'")
                }
            }
        }

        Ok(self)
    }

    /// Extract just the "By"-specific part of the tokens.
    ///
    /// For example, `name = "element-name"`.
    ///
    /// This removes the token from the vec so that we can check for leftover tokens afterwards.
    ///
    /// This will also panic if more than one such token exists.
    pub fn take_by(&mut self) -> TokenStream {
        let mut ret = Vec::new();
        let tokens_in = std::mem::take(&mut self.tokens);
        for token in tokens_in.into_iter() {
            match token {
                ByToken::Id(id) => ret.push(quote! { By::Id(#id) }),
                ByToken::Tag(tag) => ret.push(quote! { By::Tag(#tag) }),
                ByToken::LinkText(text) => ret.push(quote! { By::LinkText(#text) }),
                ByToken::Css(css) => ret.push(quote! { By::Css(#css) }),
                ByToken::XPath(xpath) => ret.push(quote! { By::XPath(#xpath) }),
                ByToken::Name(name) => ret.push(quote! { By::Name(#name) }),
                ByToken::ClassName(class_name) => ret.push(quote! { By::ClassName(#class_name) }),
                ByToken::Testid(id) => ret.push(quote! { By::Testid(#id) }),
                t => self.tokens.push(t),
            }
        }

        match ret.len() {
            0 => panic!("no selector found"),
            1 => ret.into_iter().next().unwrap(),
            _ => panic!("multiple selectors are not supported"),
        }
    }

    pub fn is_multi(&self) -> bool {
        self.tokens.iter().any(|x| matches!(&x, ByToken::Multi))
    }

    pub fn take_one<F, T>(&mut self, f: F) -> Option<T>
    where
        F: Fn(&ByToken) -> Option<T>,
    {
        let mut pos = None;
        let mut value = None;
        for (i, t) in self.tokens.iter().enumerate() {
            if let Some(v) = f(t) {
                pos = Some(i);
                value = Some(v);
                break;
            }
        }

        match (pos, value) {
            (Some(p), Some(v)) => {
                self.tokens.remove(p);
                Some(v)
            }
            _ => None,
        }
    }

    pub fn take_multi(&mut self) -> Option<bool> {
        self.take_one(|x| match x {
            ByToken::Multi => Some(true),
            _ => None,
        })
    }

    pub fn take_single(&mut self) -> Option<bool> {
        self.take_one(|x| match x {
            ByToken::Single => Some(true),
            _ => None,
        })
    }

    pub fn take_first(&mut self) -> Option<bool> {
        self.take_one(|x| match x {
            ByToken::First => Some(true),
            _ => None,
        })
    }

    pub fn take_not_empty(&mut self) -> Option<bool> {
        self.take_one(|x| match x {
            ByToken::NotEmpty => Some(true),
            _ => None,
        })
    }

    pub fn take_allow_empty(&mut self) -> Option<bool> {
        self.take_one(|x| match x {
            ByToken::AllowEmpty => Some(true),
            _ => None,
        })
    }

    pub fn take_ignore_errors(&mut self) -> Option<bool> {
        self.take_one(|x| match x {
            ByToken::IgnoreErrors => Some(true),
            _ => None,
        })
    }

    pub fn take_description(&mut self) -> Option<Literal> {
        self.take_one(|x| match x {
            ByToken::Description(d) => Some(d.clone()),
            _ => None,
        })
    }

    pub fn take_wait_options(&mut self) -> Option<WaitOptions> {
        self.take_one(|x| match x {
            ByToken::Wait(w) => Some(w.clone()),
            _ => None,
        })
    }

    pub fn take_nowait(&mut self) -> Option<bool> {
        self.take_one(|x| match x {
            ByToken::NoWait => Some(true),
            _ => None,
        })
    }

    pub fn take_custom(&mut self) -> Option<Expr> {
        self.take_one(|x| match x {
            ByToken::CustomFn(f) => Some(f.clone()),
            _ => None,
        })
    }
}

/// Parse an attribute into tokens.
///
/// This uses the above `TryFrom` impl to parse each `Meta` into `ByToken` variants.
impl TryFrom<&syn::Attribute> for ByTokens {
    type Error = syn::Error;

    fn try_from(attr: &syn::Attribute) -> Result<Self, Self::Error> {
        attr.path().is_ident("by").then_some(()).ok_or_else(|| {
            syn::Error::new(attr.span(), "only 'by' attributes are supported here")
        })?;

        let metas = attr
            .meta
            .require_list()?
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;

        metas
            .iter()
            .try_fold(
                ByTokens {
                    tokens: vec![],
                },
                |mut tokens, meta| {
                    tokens.tokens.push(ByToken::try_from(meta.clone())?);
                    tokens.validate(meta.span())
                },
            )
            .and_then(|tokens| tokens.validate(metas.span()))
    }
}

struct DebugByTokens(ByTokens);

impl ToTokens for DebugByTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        for by_token in &self.0.tokens {
            match by_token {
                // literals
                ByToken::Id(lit)
                | ByToken::Tag(lit)
                | ByToken::LinkText(lit)
                | ByToken::Css(lit)
                | ByToken::XPath(lit)
                | ByToken::Name(lit)
                | ByToken::ClassName(lit)
                | ByToken::Testid(lit)
                | ByToken::Description(lit) => lit.to_tokens(tokens),
                // idents
                ByToken::Multi
                | ByToken::NotEmpty
                | ByToken::AllowEmpty
                | ByToken::Single
                | ByToken::First
                | ByToken::IgnoreErrors
                | ByToken::NoWait => tokens.append(format_ident!("{}", by_token.get_unique_type())),
                // misc
                ByToken::CustomFn(expr) => expr.to_tokens(tokens),
                ByToken::Wait(opts) => opts.to_tokens(tokens),
            }
        }
    }
}

/// Return true if the specified path matches one of the specified types.
///
/// Use `|` as the path separator for elements in the `one_of` slice.
/// e.g. `["Vec", "std|vec|Vec"]`.
fn is_type(path: &syn::Path, one_of: &[&str]) -> bool {
    let idents: Vec<String> = path.segments.iter().map(|x| x.ident.to_string()).collect();
    let idents_of_path = idents.join("|");
    one_of.iter().any(|x| &idents_of_path == x)
}

/// Return true if this path should be treated as a multi-element resolver.
///
/// Basically any `ElementResolver<Vec<T>>` should be treated as multi.
///
/// We also catch `ElementResolverMulti` as a special case.
///
/// NOTE: If you use your own type alias for a multi-element resolver, you will need
///       to specify the `multi` attribute to force it to be treated as multi-element.
fn is_multi_resolver(path: &syn::Path) -> bool {
    // First check for the type alias.
    if path.is_ident("ElementResolverMulti") {
        true
    } else {
        if is_type(
            path,
            &[
                "ElementResolver",
                "components|ElementResolver",
                "thirtyfour|components|ElementResolver",
            ],
        ) {
            // If we have `ElementResolver<Vec<T>>` then use multi.
            let segment = path.segments.last().unwrap(); // Must be at least one.
            if let PathArguments::AngleBracketed(x) = &segment.arguments {
                for arg in &x.args {
                    if let GenericArgument::Type(syn::Type::Path(t)) = arg {
                        return is_type(
                            &t.path,
                            &["Vec", "vec|Vec", "std|vec|Vec", "alloc|vec|Vec"],
                        );
                    }
                }
            }
        }

        false
    }
}

/// All args for a single element resolver.
#[allow(clippy::large_enum_variant)]
enum SingleResolverOptions {
    CustomFn(Expr),
    Opts {
        by: TokenStream,
        first: Option<bool>,
        ignore_errors: Option<bool>,
        description: Option<Literal>,
        wait: Option<WaitOptions>,
        nowait: Option<bool>,
    },
}

/// First we convert `ByTokens` to `SingleResolverArgs`.
impl TryFrom<ByTokens> for SingleResolverOptions {
    type Error = syn::Error;
    fn try_from(mut t: ByTokens) -> Result<Self, Self::Error> {
        t.take_single(); // This is the default.
        let s = match t.take_custom() {
            Some(f) => Self::CustomFn(f),
            None => Self::Opts {
                by: t.take_by(),
                first: t.take_first(),
                ignore_errors: t.take_ignore_errors(),
                description: t.take_description(),
                wait: t.take_wait_options(),
                nowait: t.take_nowait(),
            },
        };
        if !t.tokens.is_empty() {
            return Err(syn::Error::new_spanned(DebugByTokens(t), "unexpected extra args"));
        }
        Ok(s)
    }
}

struct SingleResolverArgs {
    ty: TokenStream,
    options: SingleResolverOptions,
}

impl SingleResolverArgs {
    pub fn try_new(ty: TokenStream, by_tokens: ByTokens) -> syn::Result<Self> {
        Ok(Self {
            ty,
            options: by_tokens.try_into()?,
        })
    }
}

/// Then we convert `SingleResolverArgs` to `TokenStream`.
impl ToTokens for SingleResolverArgs {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ty = &self.ty;
        match &self.options {
            SingleResolverOptions::CustomFn(f) => {
                tokens.append_all(quote!(
                    #ty::new_custom(base.clone(), #f)
                ));
            }
            SingleResolverOptions::Opts {
                by,
                first,
                ignore_errors,
                description,
                wait,
                nowait,
            } => {
                let ignore_errors_ident = match ignore_errors {
                    Some(true) => quote!(::std::option::Option::Some(true)),
                    _ => quote!(::std::option::Option::None),
                };
                let description_ident = match description {
                    Some(desc) => {
                        quote!(::std::option::Option::Some(::std::string::ToString::to_string(&#desc)))
                    }
                    None => quote!(::std::option::Option::None),
                };
                let wait_ident = match wait {
                    Some(opts) => quote!(#opts),
                    None => match nowait {
                        Some(true) => {
                            quote! {
                                ::std::option::Option::Some(::thirtyfour::extensions::query::ElementQueryWaitOptions::NoWait)
                            }
                        }
                        _ => quote!(::std::option::Option::None),
                    },
                };
                let opts_ident = quote!(
                    ::thirtyfour::extensions::query::ElementQueryOptions::default()
                        .set_ignore_errors(#ignore_errors_ident)
                        .set_description::<String>(#description_ident)
                        .set_wait(#wait_ident)
                );

                match first {
                    Some(true) => {
                        tokens.append_all(quote!(
                            #ty::new_first_opts(base.clone(), #by, #opts_ident)
                        ));
                    }
                    _ => {
                        tokens.append_all(quote!(
                            #ty::new_single_opts(base.clone(), #by, #opts_ident)
                        ));
                    }
                }
            }
        }
    }
}

/// All args for a multi-element resolver.
#[allow(clippy::large_enum_variant)]
enum MultiResolverOptions {
    CustomFn(Expr),
    Opts {
        by: TokenStream,
        allow_empty: Option<bool>,
        ignore_errors: Option<bool>,
        description: Option<Literal>,
        wait: Option<WaitOptions>,
        nowait: Option<bool>,
    },
}

/// First we convert `ByTokens` into `MultiResolverOptions`.
impl TryFrom<ByTokens> for MultiResolverOptions {
    type Error = syn::Error;
    fn try_from(mut t: ByTokens) -> Result<Self, Self::Error> {
        t.take_multi(); // Not used here.
        t.take_not_empty(); // This is the default.
        let s = match t.take_custom() {
            Some(f) => Self::CustomFn(f),
            None => Self::Opts {
                by: t.take_by(),
                allow_empty: t.take_allow_empty(),
                ignore_errors: t.take_ignore_errors(),
                description: t.take_description(),
                wait: t.take_wait_options(),
                nowait: t.take_nowait(),
            },
        };
        if !t.tokens.is_empty() {
            return Err(syn::Error::new_spanned(DebugByTokens(t), "unexpected extra args"));
        }
        Ok(s)
    }
}

struct MultiResolverArgs {
    ty: TokenStream,
    options: MultiResolverOptions,
}

impl MultiResolverArgs {
    fn try_new(ty: TokenStream, by_tokens: ByTokens) -> syn::Result<Self> {
        Ok(Self {
            ty,
            options: by_tokens.try_into()?,
        })
    }
}

/// Then we convert `MultiResolverArgs` into `TokenStream`.
impl ToTokens for MultiResolverArgs {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ty = &self.ty;
        match &self.options {
            MultiResolverOptions::CustomFn(f) => {
                tokens.append_all(quote!(
                    #ty::new_custom(base.clone(), #f)
                ));
            }
            MultiResolverOptions::Opts {
                by,
                allow_empty,
                ignore_errors,
                description,
                wait,
                nowait,
            } => {
                let ignore_errors_ident = match ignore_errors {
                    Some(true) => quote!(::std::option::Option::Some(true)),
                    _ => quote!(::std::option::Option::None),
                };
                let description_ident = match description {
                    Some(desc) => {
                        quote!(::std::option::Option::Some(::std::string::ToString::to_string(&#desc)))
                    }
                    None => quote!(::std::option::Option::None),
                };
                let wait_ident = match wait {
                    Some(opts) => quote!(#opts),
                    None => match nowait {
                        Some(true) => {
                            quote! {
                                Some(::thirtyfour::extensions::query::ElementQueryWaitOptions::NoWait)
                            }
                        }
                        _ => quote!(None),
                    },
                };
                let opts_ident = quote!(
                    ::thirtyfour::extensions::query::ElementQueryOptions::default()
                        .set_ignore_errors(#ignore_errors_ident)
                        .set_description::<String>(#description_ident)
                        .set_wait(#wait_ident)
                );

                match allow_empty {
                    Some(true) => {
                        tokens.append_all(
                            quote!(#ty::new_allow_empty_opts(base.clone(), #by, #opts_ident)),
                        );
                    }
                    _ => {
                        tokens.append_all(quote!(
                            #ty::new_not_empty_opts(base.clone(), #by, #opts_ident)
                        ));
                    }
                }
            }
        }
    }
}

/// Converts GenericType<Args> to GenericType::<Args> to call ::new_*() on it.
///
/// Non-generic types will be returned as is.
fn fix_type(mut ty: syn::Path) -> TokenStream {
    let last = ty.segments.pop();
    match last {
        Some(pair) => {
            let (p, _) = pair.into_tuple();
            let ident = p.ident;
            let args = p.arguments;
            if args.is_empty() {
                ty.segments.push(PathSegment::from(ident));
                quote! { #ty }
            } else if ty.segments.is_empty() {
                quote! { #ident::# args }
            } else {
                quote! { #ty::#ident::#args }
            }
        }
        None => {
            quote! {}
        }
    }
}
