use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
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
            Fields::Named(ref fields) => generate_for_named_struct(fields),
            Fields::Unnamed(ref fields) => generate_for_unnamed_struct(fields),
            Fields::Unit => quote! {},
        },
        Data::Enum(enum_data) => todo!(),
        Data::Union(union_data) => todo!(),
    }
}

fn generate_for_named_struct(fields: &FieldsNamed) -> TokenStream {
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

fn generate_for_unnamed_struct(fields: &FieldsUnnamed) -> TokenStream {
    /*
     * Similar to named structs, serialize each field, but we need to enumerate over them to get the indices into
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
