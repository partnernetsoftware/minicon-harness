// The version-source court (W4): every place that spells the product version
// must spell the same one.
//
// The repository has bitten itself before with two version strings in one
// artifact, so this is a test rather than a review note: it reads the files
// that packaging and signing actually consume and fails on any disagreement
// with `CARGO_PKG_VERSION`.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(relative: &str) -> String {
    let path = root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[test]
fn every_version_source_agrees() {
    // The crate itself.
    assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));

    // The loader's Windows resource: both FileVersion and ProductVersion.
    let rc = read("loader/ape-version.rc");
    assert!(
        rc.contains(&format!("VALUE \"FileVersion\", \"{VERSION}\"")),
        "ape-version.rc FileVersion does not match {VERSION}"
    );
    assert!(
        rc.contains(&format!("VALUE \"ProductVersion\", \"{VERSION}\"")),
        "ape-version.rc ProductVersion does not match {VERSION}"
    );

    // The release policy the packaging step reads.
    let policy = read("release-policy.json");
    let parsed: serde_json::Value = serde_json::from_str(&policy).expect("release-policy.json is JSON");
    assert_eq!(
        parsed["version"].as_str(),
        Some(VERSION),
        "release-policy.json version does not match {VERSION}"
    );

    // Cargo.toml is the origin of CARGO_PKG_VERSION; assert it explicitly so a
    // future workspace split cannot move the version somewhere unread.
    let manifest = read("Cargo.toml");
    assert!(
        manifest.contains(&format!("version = \"{VERSION}\"")),
        "Cargo.toml does not declare {VERSION}"
    );
}

#[test]
fn the_pack_script_accepts_only_three_part_versions() {
    // pack.sh derives the comma form and asserts it; the regex it uses must
    // accept this version, or the release chain fails late.
    let parts: Vec<&str> = VERSION.split('.').collect();
    assert_eq!(parts.len(), 3, "the version must be X.Y.Z for pack.sh, got {VERSION}");
    for part in parts {
        assert!(
            !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()),
            "every component must be numeric, got {VERSION}"
        );
    }
}

#[test]
fn the_gate_banner_names_the_same_version() {
    assert_eq!(
        minicon_harness::gate::VERSION_BANNER,
        format!("minicon-gate {VERSION}\n")
    );
    assert_eq!(minicon_harness::VERSION_BANNER, format!("minicon-harness {VERSION}\n"));
}

#[test]
fn the_release_policy_still_asks_for_the_two_sidecars() {
    // The 0.0.19 gap was that the adapter was built but never archived. The
    // policy is where an asset is declared, so its shape is asserted here.
    let parsed: serde_json::Value =
        serde_json::from_str(&read("release-policy.json")).expect("release-policy.json is JSON");
    assert_eq!(parsed["schema"].as_u64(), Some(1));
    assert_eq!(parsed["assets"]["native_archives"].as_bool(), Some(true));
    assert_eq!(parsed["assets"]["minicon_harness_com"].as_bool(), Some(true));
    assert_eq!(parsed["signing"]["mode"].as_str(), Some("required"));
}
