fn main() -> miette::Result<()> {
    env_logger::init();

    // let src = std::fs::read_to_string("./schema/1.musicxml").unwrap();
    let src = std::fs::read_to_string("./schema/ODDTAXI.musicxml").unwrap();
    let conversion = neoclef::parse_full(&src, false)?;
    for feature in conversion.unsupported.as_set() {
        eprintln!("unsupported: {feature}");
    }
    conversion.smf.save("out.mid").unwrap();
    Ok(())
}
