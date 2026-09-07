fn main() {
    println!("cargo:rerun-if-env-changed=IDENTITY_BUILD_REVISION");
    println!("cargo:rerun-if-changed=../../.git/index");
    let revision = std::env::var("IDENTITY_BUILD_REVISION")
        .ok()
        .or_else(|| {
            let output = std::process::Command::new("git")
                .args(["rev-parse", "--short=12", "HEAD"])
                .output()
                .ok()?;
            output.status.success().then(|| {
                let revision = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                let dirty = std::process::Command::new("git")
                    .args(["status", "--porcelain"])
                    .output()
                    .is_ok_and(|output| !output.stdout.is_empty());
                format!("{revision}{}", if dirty { "-dirty" } else { "" })
            })
        })
        .unwrap_or_else(|| "unversioned".into());
    println!("cargo:rustc-env=IDENTITY_BUILD_REVISION={revision}");
}
