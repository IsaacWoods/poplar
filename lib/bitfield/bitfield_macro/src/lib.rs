use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Attribute, Ident, LitInt, Token, Type, Visibility, braced,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

struct BitfieldRepr {
    attrs: Vec<Attribute>,
    vis: Visibility,
    _struct_token: Token![struct],
    ident: Ident,

    _lt_token: Token![<],
    base: Type,
    _gt_token: Token![>],

    _brace_token: syn::token::Brace,
    fields: Punctuated<Field, Token![;]>,
}

impl Parse for BitfieldRepr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(BitfieldRepr {
            attrs: input.call(Attribute::parse_outer)?,
            vis: input.parse()?,
            _struct_token: input.parse()?,
            ident: input.parse()?,

            _lt_token: input.parse()?,
            base: input.parse()?,
            _gt_token: input.parse()?,

            _brace_token: braced!(content in input),
            fields: content.parse_terminated(Field::parse, Token![;])?,
        })
    }
}

struct Field {
    attrs: Vec<Attribute>,
    vis: Visibility,
    name: Ident,
    width: u32,
}

impl Parse for Field {
    fn parse(input: ParseStream) -> syn::Result<Field> {
        let attrs = input.call(Attribute::parse_outer)?;

        let vis: Visibility = input.parse()?;
        input.parse::<Token![const]>()?;
        let name: Ident = input.parse()?;

        let width = if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;
            let ty: Type = input.parse()?;
            if let Type::Path(p) = &ty
                && p.path.is_ident("bool")
            {
                1
            } else {
                return Err(syn::Error::new_spanned(ty, "typed field must be a `bool`"));
            }
        } else if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            let n: LitInt = input.parse()?;
            n.base10_parse()?
        } else {
            return Err(input.error("expected `=` or `:`"));
        };

        Ok(Field {
            attrs,
            vis,
            name,
            width,
        })
    }
}

#[proc_macro]
pub fn bitfield(item: TokenStream) -> TokenStream {
    let BitfieldRepr {
        attrs,
        vis,
        ident,
        base,
        fields,
        ..
    } = syn::parse_macro_input!(item as BitfieldRepr);

    let mut field_consts = Vec::new();
    if let Some(Field {
        attrs,
        vis,
        name,
        width,
    }) = fields.first()
    {
        field_consts.push(quote! { #( #attrs )* #vis const #name: ::bitfield::pack::Packer<#base> = ::bitfield::pack::Packer::least_significant(#width); });
    }
    for i in 1..fields.len() {
        let Field {
            name: prev_name, ..
        } = &fields[i - 1];
        let Field {
            attrs,
            vis,
            name,
            width,
        } = &fields[i];
        field_consts.push(
            quote! { #( #attrs )* #vis const #name: ::bitfield::pack::Packer<#base> = Self::#prev_name.next(#width); },
        );
    }

    quote! {
        #( #attrs )*
        #[repr(transparent)]
        #[derive(Clone, Copy, PartialEq, Eq)]
        #vis struct #ident(pub #base);

        impl #ident {
            #(
                #field_consts
            )*

            #vis const fn new() -> Self {
                Self(0)
            }

            #vis const fn from_raw(value: #base) -> Self {
                Self(value)
            }

            #vis const fn with<T>(self, field: ::bitfield::pack::Packer<#base>, value: T) -> Self where T: const ::bitfield::FromBits<#base> {
                Self(field.pack(self.0, value))
            }

            #vis fn get<T>(&self, field: ::bitfield::pack::Packer<#base>) -> T where T: const ::bitfield::FromBits<#base> {
                field.unpack(self.0)
            }
        }
    }
    .into()
}
