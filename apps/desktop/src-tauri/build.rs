fn main() {
    println!("cargo:rerun-if-env-changed=TMDB_READ_ACCESS_TOKEN");
    println!("cargo:rerun-if-env-changed=TMDB_API_KEY");

    let release_build = std::env::var("PROFILE").is_ok_and(|profile| profile == "release");
    let tmdb_configured = ["TMDB_READ_ACCESS_TOKEN", "TMDB_API_KEY"]
        .into_iter()
        .any(|name| std::env::var(name).is_ok_and(|value| !value.trim().is_empty()));

    assert!(
        !release_build || tmdb_configured,
        "CINE WANA release build blocked: TMDB is not configured. Use the canonical `npm.cmd run desktop:build` command with TMDB_READ_ACCESS_TOKEN or TMDB_API_KEY present in the ignored root .env file."
    );

    tauri_build::build()
}
