// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn attach_takes_replace() {
    let cli = Cli::try_parse_from(["xmorpheo", "attach", "--all", "--replace"]).unwrap();
    assert!(matches!(
        cli.command,
        Command::Attach {
            all: true,
            replace: true,
            ..
        }
    ));
    let cli = Cli::try_parse_from(["xmorpheo", "attach", "231d:0200"]).unwrap();
    assert!(matches!(
        cli.command,
        Command::Attach { replace: false, .. }
    ));
}
