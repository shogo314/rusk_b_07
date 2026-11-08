mod support;
use support::Case;

fn check_samples(id: &str, source: &str) {
    let catalog: serde_json::Value =
        serde_json::from_str(include_str!("../examples/atcoder/samples.json")).unwrap();
    let task = &catalog[id];
    let samples = task["samples"].as_array().expect("sample list");
    assert!(!samples.is_empty(), "{id}: no samples");
    let case = Case::new(source);
    for (index, sample) in samples.iter().enumerate() {
        let input = sample["input"].as_str().expect("sample input");
        let expected = sample["output"].as_str().expect("sample output");
        // Compile each solution once, then reuse it for every official sample.
        let out = if index == 0 {
            case.run(input)
        } else {
            case.execute(input)
        };
        assert!(
            out.status.success(),
            "{id} sample {}: {}",
            index + 1,
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            out.stderr.is_empty(),
            "{id} sample {}: {}",
            index + 1,
            String::from_utf8_lossy(&out.stderr)
        );
        let actual = std::str::from_utf8(&out.stdout).expect("UTF-8 output");
        assert_eq!(
            actual.split_whitespace().collect::<Vec<_>>(),
            expected.split_whitespace().collect::<Vec<_>>(),
            "{id} sample {} ({})",
            index + 1,
            task["url"]
        );
    }
}

macro_rules! atcoder {
    ($($id:ident),* $(,)?) => {
        const TASKS: &[&str] = &[$(stringify!($id)),*];
        $(
        #[test]
        fn $id() {
            check_samples(stringify!($id), include_str!(concat!("../examples/atcoder/", stringify!($id), ".rusk")));
        }
    )*};
}

#[test]
fn sample_catalog_covers_every_solution() {
    let catalog: serde_json::Value =
        serde_json::from_str(include_str!("../examples/atcoder/samples.json")).unwrap();
    let registered: std::collections::BTreeSet<_> = TASKS.iter().copied().collect();
    let fixtures: std::collections::BTreeSet<_> = catalog
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(registered, fixtures, "every fixture must have a test");
    let sources: std::collections::BTreeSet<_> =
        std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/atcoder"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rusk"))
            .map(|path| path.file_stem().unwrap().to_str().unwrap().to_owned())
            .collect();
    assert_eq!(
        registered,
        sources.iter().map(String::as_str).collect(),
        "every solution must have a test"
    );
}

atcoder!(
    abc050_b, abc079_b, abc080_b, abc081_a, abc081_b, abc083_b, abc084_b, abc085_a, abc085_b,
    abc086_a, abc086_b, abc087_a, abc087_b, abc088_a, abc088_b, abc089_b, abc090_b, abc092_b,
    abc094_b, abc095_a, abc095_b, abc096_b, abc097_b, abc098_b, abc099_b, abc100_b, abc111_b,
    abc125_a, abc134_c, abc136_b, abc143_d, abc146_c, abc163_b, abc163_c, abc174_e, abc175_b,
    abc176_b, abc178_e, abc179_b, abc179_c, abc180_c, abc181_c, abc184_b, abc186_b, abc186_c,
    abc187_b, abc188_b, abc190_b, abc191_b, abc192_b, abc193_b, abc194_b, abc194_c, abc196_c,
    abc199_b, abc200_b, abc202_b, abc205_b, abc206_b, abc206_c, abc208_b, abc211_b, abc213_b,
    abc214_b, abc215_b, abc217_b, abc221_b, abc226_b, abc227_b, abc228_b, abc229_b, abc232_b,
    abc233_b, abc235_b, abc236_b, abc237_c, abc238_b, abc239_b, abc240_c, abc241_b, abc244_b,
    abc248_b, abc249_b, abc250_b, abc252_b, abc256_b, abc257_b, abc258_b, abc261_b, abc263_b,
    abc265_b, abc271_b, abc272_b, abc289_b, abc293_b, abc305_c, abc320_b, abc332_b, abc373_b,
    abc373_c,
);
