fn main() -> miette::Result<()> {
    env_logger::init();

    // let src = std::fs::read_to_string("./schema/1.musicxml").unwrap();
    let src = std::fs::read_to_string("./schema/ODDTAXI.musicxml").unwrap();
    let smf = neoclef::parse(&src, false)?;
    smf.save("out.mid").unwrap();
    Ok(())
}
