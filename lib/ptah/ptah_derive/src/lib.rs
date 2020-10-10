use proc_macro::TokenStream;

#[proc_macro_derive(Serialize, attributes(ptah))]
pub fn derive_serialize(input: TokenStream) -> TokenStream {
    todo!()
}

#[proc_macro_derive(Deserialize, attributes(ptah))]
pub fn derive_deserialize(input: TokenStream) -> TokenStream {
    todo!()
}
