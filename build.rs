use vergen::{generate_cargo_keys, ConstantsFlags};
use winres;

fn main() {
    generate_cargo_keys(ConstantsFlags::all()).expect("Unable to generate the cargo keys!");

    let mut res = winres::WindowsResource::new();
    res.set_icon("compact.ico");
    res.compile().unwrap();
}
