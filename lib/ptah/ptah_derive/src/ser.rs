use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::{
    parse_quote,
    spanned::Spanned,
    Data,
    DataEnum,
    DeriveInput,
    Fields,
    FieldsNamed,
    FieldsUnnamed,
    GenericParam,
    Generics,
    Index,
};

// TODO: work out how to throw errors properly (apparently there's an experimental Diagnostics API?)
// Serde doesn't use it but it might just not have been updated yet / waiting for it to be stable
pub fn impl_serialize(input: DeriveInput) -> proc_macro::TokenStream {
    let name = input.ident;

    let generics = add_trait_bounds(input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let body = generate_body(&input.data);

    let expanded = quote! {
        #[automatically_derived]
        impl #impl_generics ptah::Serialize for #name #ty_generics #where_clause {
            fn serialize<W>(&self, serializer: &mut ptah::Serializer<W>) -> ptah::ser::Result<()>
            where
                W: ptah::Writer,
            {
                #body
                Ok(())
            }
        }
    };
    proc_macro::TokenStream::from(expanded)
}

/*
 * This adds a bound `T: ptah::Serialize` to each type parameter `T`.
 */
fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(ptah::Serialize));
        }
    }
    generics
}

fn generate_body(data: &Data) -> TokenStream {
    match data {
        Data::Struct(ref struct_data) => match struct_data.fields {
            Fields::Named(ref fields) => generate_for_struct(fields),
            Fields::Unnamed(ref fields) => generate_for_tuple(fields),
            Fields::Unit => quote! {},
        },
        Data::Enum(enum_data) => generate_for_enum(&enum_data),
        // TODO: I'm not sure we want to support this from Rust. Probably throw a compile error here?
        Data::Union(union_data) => todo!(),
    }
}

fn generate_for_struct(fields: &FieldsNamed) -> TokenStream {
    /*
     * We serialise each field, making sure to use fully-qualified syntax so we don't need the
     * traits in-scope. We also make sure to match each field back to its correct span, so we get
     * nice error messages.
     */
    let fields = fields.named.iter().map(|field| {
        let name = &field.ident;
        quote_spanned!(field.span() => ptah::Serialize::serialize(&self.#name, serializer)?;)
    });
    quote! {
        #(#fields)*
    }
}

fn generate_for_tuple(fields: &FieldsUnnamed) -> TokenStream {
    /*
     * Similar to named fields, serialize each one, but we need to enumerate over them to get the indices into
     * the tuple, as the field doesn't contain it.
     */
    let fields = fields.unnamed.iter().enumerate().map(|(i, field)| {
        let index = Index::from(i);
        quote_spanned!(field.span() => ptah::Serialize::serialize(&self.#index, serializer)?;)
    });

    quote! {
        #(#fields)*
    }
}

fn generate_for_enum(data: &DataEnum) -> TokenStream {
    let variants = data.variants.iter().enumerate().map(|(i, variant)| {
        // TODO: we should probably handle explicit descriminants (e.g. SomeVariant = 78,) somehow
        assert!(variant.discriminant.is_none());

        let index = Index::from(i);
        let variant_name = &variant.ident;

        match &variant.fields {
            Fields::Named(ref fields) => {
                let serialize_fields = fields.named.iter().map(|field| {
                    let field_name = &field.ident;
                    quote_spanned!(variant.span() => ptah::Serialize::serialize(#field_name, serializer)?;)
                });
                let field_names = fields.named.iter().map(|field| {
                    let field_name = &field.ident;
                    quote!(#field_name, )
                });

                quote_spanned!(variant.span() => Self::#variant_name { #(#field_names)* } => {
                    ptah::Serializer::serialize_enum_variant(serializer, #index)?;
                    #(#serialize_fields)*
                })
            }
            Fields::Unnamed(ref fields) => {
                let field_names = fields.unnamed.iter().enumerate().map(|(i, _field)| {
                    let field_name = format_ident!("field_{}", i);
                    quote!(#field_name, )
                });
                let serialize_fields = fields.unnamed.iter().enumerate().map(|(i, field)| {
                    let field_name = format_ident!("field_{}", i);
                    quote_spanned!(field.span() => ptah::Serialize::serialize(#field_name, serializer)?;)
                });

                quote_spanned!(variant.span() => Self::#variant_name(#(#field_names)*) => {
                    ptah::Serializer::serialize_enum_variant(serializer, #index)?;
                    #(#serialize_fields)*
                })
            }
            Fields::Unit => quote_spanned!(variant.span() => Self::#variant_name => {
                ptah::Serializer::serialize_enum_variant(serializer, #index)?;
            }),
        }
    });

    quote! {
        match self {
            #(#variants)*
        }
    }
}
