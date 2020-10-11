use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::{
    parse_quote,
    spanned::Spanned,
    Data,
    DeriveInput,
    Fields,
    FieldsNamed,
    FieldsUnnamed,
    GenericParam,
    Generics,
    Ident,
};

// TODO: work out how to throw errors properly (apparently there's an experimental Diagnostics API?)
// Serde doesn't use it but it might just not have been updated yet / waiting for it to be stable
pub fn impl_deserialize(input: DeriveInput) -> proc_macro::TokenStream {
    let name = input.ident;
    let body = generate_body(&name, &input.data);

    /*
     * We need to split the generics into different parts that can be `quote!`ed to produce the `impl` block. We
     * need to add a new lifetime `'de`, but we can't just add it to the `Generics` as you might expect because
     * this adds it to the type (e.g. it emits `Foo<'de>`), so we have to do... this.
     *
     * We call the lifetime `'_de` to reduce the chance it collides with a lifetime on the type.
     */
    let generics = add_trait_bounds(input.generics);
    let generics_with_de_lifetime = {
        let mut generics_with_lifetime = generics.clone();
        generics_with_lifetime.params.push(parse_quote!('_de));
        generics_with_lifetime
    };
    let (impl_generics, ty_generics, where_clause) = {
        let (impl_generics, _, _) = generics_with_de_lifetime.split_for_impl();
        let (_, ty_generics, where_clause) = generics.split_for_impl();
        (impl_generics, ty_generics, where_clause)
    };

    let expanded = quote! {
        #[automatically_derived]
        impl #impl_generics ptah::Deserialize<'_de> for #name #ty_generics #where_clause {
            fn deserialize(deserializer: &mut ptah::Deserializer<'_de>) -> ptah::de::Result<Self> {
                #body
            }
        }
    };
    proc_macro::TokenStream::from(expanded)
}

/*
 * This adds a bound `T: ptah::Deserialize` to each type parameter `T`.
 */
fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(ptah::Deserialize));
        }
    }
    generics
}

fn generate_body(struct_name: &Ident, data: &Data) -> TokenStream {
    match data {
        Data::Struct(ref struct_data) => match struct_data.fields {
            Fields::Named(ref fields) => generate_for_named_struct(struct_name, fields),
            Fields::Unnamed(ref fields) => generate_for_unnamed_struct(struct_name, fields),
            Fields::Unit => quote! {},
        },
        Data::Enum(enum_data) => todo!(),
        Data::Union(union_data) => todo!(),
    }
}

fn generate_for_named_struct(struct_name: &Ident, fields: &FieldsNamed) -> TokenStream {
    /*
     * First, we deserialize each field into a local, in order. We make sure to use fully-qualified syntax to
     * access `Deserialize`, and to match each field back to its correct span, so we get nice error messages.
     */
    let deserialize_each = fields.named.iter().map(|field| {
        let name = &field.ident;
        let field_type = &field.ty;
        quote_spanned!(field.span() => let #name: #field_type = ptah::Deserialize::deserialize(deserializer)?;)
    });

    let struct_init = fields.named.iter().map(|field| {
        let name = &field.ident;
        quote!(#name, )
    });

    quote! {
        #(#deserialize_each)*
        Ok(#struct_name { #(#struct_init)* })
    }
}

fn generate_for_unnamed_struct(struct_name: &Ident, fields: &FieldsUnnamed) -> TokenStream {
    let deserialize_each = fields.unnamed.iter().enumerate().map(|(i, field)| {
        let field_name = format_ident!("field_{}", i);
        let field_type = &field.ty;
        quote_spanned!(field.span() => let #field_name: #field_type = ptah::Deserialize::deserialize(deserializer)?;)
    });

    let struct_init = fields.unnamed.iter().enumerate().map(|(i, _field)| {
        let field_name = format_ident!("field_{}", i);
        quote!(#field_name, )
    });

    quote! {
        #(#deserialize_each)*
        Ok(#struct_name(#(#struct_init)*))
    }
}
