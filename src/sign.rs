use std::process::Command;

pub fn sign() -> Result<(), String> {
    if check_xcode_installation().is_err() {
        return Err("No valid XCode installation detected. Aborting.".into());
    }
    Ok(())
}

fn check_xcode_installation() -> Result<(), ()> {
    let xcode_installation = Command::new("/usr/bin/xcode-select").arg("-p").status();

    if xcode_installation.is_err() || !xcode_installation.unwrap().success() {
        println!("Failed to detect XCode command-line tools. XCode command-line tools are needed to sign IPA files.");
        println!("Install XCode? (Y/n)");

        let stdin = std::io::stdin();
        let mut buffer = String::new();
        let result = loop {
            buffer.clear();
            stdin.read_line(&mut buffer).unwrap();
            buffer = buffer.to_lowercase();
            if buffer.starts_with('y') {
                break true;
            } else if buffer.starts_with('n') {
                break false;
            }
        };

        if result {
            let installation_result = Command::new("/usr/bin/xcode-select")
                .arg("--install")
                .status();
            if let Ok(result) = installation_result {
                if result.success() {
                    Ok(())
                } else {
                    Err(())
                }
            } else {
                Err(())
            }
        } else {
            println!("No XCode installation detected, stopping.");
            Err(())
        }
    } else {
        Ok(())
    }
}
