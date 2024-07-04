#![feature(proc_macro_span)]

extern crate proc_macro;
use proc_macro::*;

/// Defines a unique identifier for the structure.
///
/// # Example
///
/// ```
/// [identify("12345678-1234-5678-abcd-123456789abc")]
/// pub struct Foo {}
/// ```
#[proc_macro_attribute]
pub fn identify(attr: TokenStream, item: TokenStream) -> TokenStream {
    fn unexpected_eof(stream: TokenStream) -> ! {
        panic!(
            "Unexpected eof {:?}",
            stream.into_iter().next().unwrap().span().end()
        )
    }

    fn unexpected_token(token: TokenTree, expected: &str) -> ! {
        panic!("Expected {}, but {:?}", expected, token)
    }

    let Some(token) = attr.clone().into_iter().next() else {
        unexpected_eof(attr)
    };
    let uuid_string = match token {
        TokenTree::Literal(literal) => literal.to_string(),
        _ => unexpected_token(token, "uuid_string"),
    };

    let mut tokens = item.clone().into_iter();
    loop {
        let Some(token) = tokens.next() else {
            unexpected_eof(item)
        };
        match token {
            TokenTree::Ident(ident) => match ident.to_string().as_str() {
                "struct" => break,
                _ => {}
            },
            _ => {}
        }
    }
    let Some(token) = tokens.next() else {
        unexpected_eof(item)
    };
    let ident = match token {
        TokenTree::Ident(ident) => ident.to_string(),
        _ => unexpected_token(token, "ident"),
    };

    let uuid_a = &uuid_string[1..9];
    let uuid_b = &uuid_string[10..14];
    let uuid_c = &uuid_string[15..19];
    let uuid_d = &uuid_string[20..24];
    let uuid_e0 = &uuid_string[25..27];
    let uuid_e1 = &uuid_string[27..29];
    let uuid_e2 = &uuid_string[29..31];
    let uuid_e3 = &uuid_string[31..33];
    let uuid_e4 = &uuid_string[33..35];
    let uuid_e5 = &uuid_string[35..37];

    let insert = format!(
        "
unsafe impl Identify for {ident} {{
    const UUID: Uuid = Uuid::from_parts(0x{uuid_a}, 0x{uuid_b}, 0x{uuid_c}, 0x{uuid_d}, [0x{uuid_e0},0x{uuid_e1},0x{uuid_e2},0x{uuid_e3},0x{uuid_e4},0x{uuid_e5}]);
}}
"
    );

    let mut item = item;
    item.extend(insert.parse::<TokenStream>().unwrap().into_iter());

    item
}
