// SPDX-License-Identifier: GPL-2.0-or-later

use xmorpheo::config::*;
use xmorpheo::curve::AxisKind;
use xmorpheo::descriptor::Usage;
use xmorpheo::{curve, formula};

const X: Usage = Usage::new(0x01, 0x30);
const RZ: Usage = Usage::new(0x01, 0x35);

#[test]
fn vkb_example() {
    let config = Config::parse(include_str!("../../examples/vkb-gladiator-evo.toml")).unwrap();
    assert_eq!(config.devices.len(), 2);
    let products: Vec<_> = config.devices.iter().map(|d| d.product).collect();
    assert_eq!(products, [0x0201, 0x0200]);
    for dev in &config.devices {
        assert_eq!(dev.vendor, 0x231d);
        assert_eq!(dev.uniq, None);
        let usages: Vec<_> = dev.axes.iter().map(|a| a.usage).collect();
        assert_eq!(usages, [X, Usage::new(0x01, 0x31), RZ]);
        for axis in &dev.axes {
            assert_eq!(axis.kind, AxisKind::Centered);
            assert_eq!(axis.tolerance, curve::DEFAULT_TOLERANCE);
            assert_eq!(axis.curve.eval(&[1.0]), 1.0);
            assert_eq!(axis.curve.eval(&[-1.0]), -1.0);
            assert_eq!(axis.terms.len(), 1);
        }
        // k_stick = 0.6 for X, k_twist = 0.75 for Rz.
        assert!((dev.axes[0].curve.eval(&[0.5]) - 0.275).abs() < 1e-9);
        assert!((dev.axes[2].curve.eval(&[0.5]) - 0.21875).abs() < 1e-9);
    }
}

#[test]
fn xbox_example() {
    let config =
        Config::parse(include_str!("../../examples/xbox-wireless-controller.toml")).unwrap();
    let dev = &config.devices[0];
    assert_eq!(
        (dev.vendor, dev.product, dev.keep_evdev),
        (0x045e, 0x0b13, true)
    );
    assert_eq!(dev.axes.len(), 4);
    // The commented trigger curves parse too.
    let triggers = include_str!("../../examples/xbox-wireless-controller.toml")
        .lines()
        .filter_map(|l| l.strip_prefix("# "))
        .skip_while(|l| !l.starts_with("[[device.axis]]"))
        .collect::<Vec<_>>()
        .join("\n");
    let config = Config::parse(&format!(
        "[[device]]\nvendor = 1\nproduct = 2\n{triggers}\n"
    ))
    .unwrap();
    let kinds: Vec<_> = config.devices[0].axes.iter().map(|a| a.kind).collect();
    assert_eq!(kinds, [AxisKind::Unipolar, AxisKind::Unipolar]);
}

#[test]
fn xbox_combined_triggers_example() {
    let config = Config::parse(include_str!("../../examples/xbox-combined-triggers.toml")).unwrap();
    let dev = &config.devices[0];
    assert_eq!(dev.axes.len(), 6);
    let z = &dev.axes[4];
    assert_eq!((z.kind, z.terms.len()), (AxisKind::Centered, 2));
    assert!(z.sources.iter().all(|s| s.kind == AxisKind::Unipolar));
}

#[test]
fn design_example() {
    let design = include_str!("../../docs/DESIGN.md");
    let start = design.find("```toml\n").unwrap() + "```toml\n".len();
    let len = design[start..].find("```").unwrap();
    let config = Config::parse(&design[start..start + len]).unwrap();
    assert_eq!(config.devices[0].axes.len(), 2);
}

#[test]
fn options() {
    let config = Config::parse(
        r#"
        tolerance = 0.25

        [[device]]
        vendor = 0x044f
        product = 0xb10a
        uniq = "abc"

        [[device.axis]]
        usage = "Throttle"
        curve = "x"

        [[device.axis]]
        usage = "Z"
        curve = "x"
        kind = "unipolar"
        tolerance = 0

        [[device.axis]]
        usage = "0002:00c5"
        curve = "pow(x, 2)"
        "#,
    )
    .unwrap();
    let dev = &config.devices[0];
    assert_eq!(dev.name, None);
    assert_eq!(dev.uniq.as_deref(), Some("abc"));
    let axes: Vec<_> = dev.axes.iter().map(|a| (a.kind, a.tolerance)).collect();
    assert_eq!(
        axes,
        [
            (AxisKind::Unipolar, 0.25),
            (AxisKind::Unipolar, 0.0),
            (AxisKind::Unipolar, 0.25),
        ]
    );
}

#[test]
fn find() {
    let config = Config::parse(
        "[[device]]\nvendor = 1\nproduct = 2\nuniq = \"a\"\nname = \"a\"\n\
         [[device]]\nvendor = 1\nproduct = 2\n",
    )
    .unwrap();
    let name = |uniq| config.find(1, 2, uniq, "").map(|d| d.name.is_some());
    assert_eq!(name("a"), Some(true));
    assert_eq!(name("b"), Some(false));
    assert_eq!(name(""), Some(false));
    assert!(config.find(1, 3, "a", "").is_none());
}

/// An entry that names the driver only matches with that driver, and wins
/// over an entry without a driver, wherever that one is.
#[test]
fn find_by_driver() {
    let config = Config::parse(
        "[[device]]\nvendor = 1\nproduct = 2\nname = \"any\"\n\
         [[device]]\nvendor = 1\nproduct = 2\ndriver = \"tmff2\"\nname = \"tmff2\"\n\
         [[device]]\nvendor = 1\nproduct = 3\ndriver = \"tmff2\"\n",
    )
    .unwrap();
    assert_eq!(config.devices[1].driver.as_deref(), Some("tmff2"));
    let name = |product, driver| {
        config
            .find(1, product, "", driver)
            .and_then(|d| d.name.as_deref())
    };
    assert_eq!(name(2, "tmff2"), Some("tmff2"));
    assert_eq!(name(2, "hid-generic"), Some("any"));
    assert_eq!(name(2, ""), Some("any"));
    assert!(config.find(1, 3, "", "hid-generic").is_none());
    assert!(config.find(1, 3, "", "tmff2").is_some());
}

#[test]
fn evdev_tables() {
    let config = Config::parse(
        "[[device]]\nvendor = 1\nproduct = 2\n\
         [device.fuzz]\nABS_Z = 4\n[device.flat]\nABS_X = 3072\nABS_FOO = 1\n",
    )
    .unwrap();
    let dev = &config.devices[0];
    assert_eq!(dev.fuzz.get("ABS_Z"), Some(&4));
    // Names are checked against the evdev node, not here.
    assert_eq!(dev.flat.len(), 2);
    assert!(matches!(
        Config::parse("[[device]]\nvendor = 1\nproduct = 2\n[device.flat]\nABS_X = -1\n"),
        Err(Error::Toml(_))
    ));
    let keep = Config::parse("[[device]]\nvendor = 1\nproduct = 2\nevdev = \"keep\"\n").unwrap();
    assert!(keep.devices[0].keep_evdev);
    assert!(!config.devices[0].keep_evdev);
    assert_eq!(
        Config::parse("[[device]]\nvendor = 1\nproduct = 2\nevdev = \"set\"\n"),
        Err(Error::InvalidEvdev {
            device: 1,
            value: "set".to_string()
        })
    );
    assert_eq!(
        Config::parse(
            "[[device]]\nvendor = 1\nproduct = 2\nevdev = \"keep\"\n[device.flat]\nABS_X = 1\n"
        ),
        Err(Error::KeptEvdevWithValues { device: 1 })
    );
}

const COMBINED: &str = r#"
    [[device]]
    vendor = 0x045e
    product = 0x0b13

    [[device.axis]]
    usage = "Z"
    kind = "centered"
    source.r = { usage = "Rz", kind = "unipolar" }
    source.l = { usage = "Z", kind = "unipolar" }
    curve = "r - l"

    [[device.axis]]
    usage = "Rz"
    curve = "0"
"#;

#[test]
fn sources() {
    let config = Config::parse(COMBINED).unwrap();
    let z = &config.devices[0].axes[0];
    let names: Vec<_> = z
        .sources
        .iter()
        .map(|s| (s.name.as_str(), s.usage, s.kind))
        .collect();
    // The variables come in the order of their names.
    assert_eq!(
        names,
        [
            ("l", Usage::new(0x01, 0x32), AxisKind::Unipolar),
            ("r", Usage::new(0x01, 0x35), AxisKind::Unipolar),
        ]
    );
    let terms: Vec<_> = z
        .terms
        .iter()
        .map(|t| (t.var, t.curve.to_string()))
        .collect();
    assert_eq!(terms, [(0, "(-x)".to_string()), (1, "x".to_string())]);
    assert_eq!(z.kind, AxisKind::Centered);
    // Without a source table, x is the axis itself, with its kind.
    let rz = &config.devices[0].axes[1];
    assert_eq!(rz.sources.len(), 1);
    assert_eq!(
        (rz.sources[0].name.as_str(), rz.sources[0].usage),
        ("x", Usage::new(0x01, 0x35))
    );
    assert!(config.warnings.is_empty());
}

#[test]
fn source_short_form_takes_the_kind_of_the_usage() {
    let config = Config::parse(
        "[[device]]\nvendor = 1\nproduct = 2\n[[device.axis]]\nusage = \"X\"\n\
         source.t = \"Throttle\"\nsource.y = \"Y\"\ncurve = \"t + y\"\n",
    )
    .unwrap();
    let kinds: Vec<_> = config.devices[0].axes[0]
        .sources
        .iter()
        .map(|s| s.kind)
        .collect();
    assert_eq!(kinds, [AxisKind::Unipolar, AxisKind::Centered]);
}

#[test]
fn source_errors() {
    let axis = |source: &str, curve: &str| {
        format!(
            "[constants]\nk = 1\n[[device]]\nvendor = 1\nproduct = 2\n[[device.axis]]\n\
             usage = \"X\"\n{source}\ncurve = \"{curve}\"\n"
        )
    };
    for name in ["pow", "k", "1a", "a-b"] {
        assert!(
            matches!(
                Config::parse(&axis(&format!("source.\"{name}\" = \"Y\""), "x")),
                Err(Error::InvalidVariable { .. })
            ),
            "{name}"
        );
    }
    // Formulas only see the variables of the source table.
    assert!(matches!(
        Config::parse(&axis("source.y = \"Y\"", "x + y")),
        Err(Error::Formula {
            error: formula::Error::UnknownName { .. },
            ..
        })
    ));
    assert!(matches!(
        Config::parse(&axis("source.a = \"Y\"\nsource.b = \"Z\"", "a * b")),
        Err(Error::Formula {
            error: formula::Error::NotSeparable(_),
            ..
        })
    ));
    assert!(matches!(
        Config::parse(&axis("source.a = \"Foo\"", "a")),
        Err(Error::UnknownUsage { .. })
    ));
    assert!(matches!(
        Config::parse(&axis("source.a = { usage = \"Y\", kind = \"up\" }", "a")),
        Err(Error::Toml(_))
    ));
    // Unused variables are only a warning.
    let config = Config::parse(&axis("source.a = \"Y\"\nsource.b = \"Z\"", "a")).unwrap();
    assert_eq!(
        config.warnings,
        ["device 1, axis X: variable b is not used"]
    );
}

#[test]
fn empty() {
    assert_eq!(
        Config::parse(""),
        Ok(Config {
            devices: vec![],
            warnings: vec![]
        })
    );
}

#[test]
fn errors() {
    let axis = |usage: &str, curve: &str| {
        format!(
            "[[device]]\nvendor = 1\nproduct = 2\n[[device.axis]]\nusage = \"{usage}\"\ncurve = \"{curve}\"\n"
        )
    };
    assert_eq!(
        Config::parse(&axis("Foo", "x")),
        Err(Error::UnknownUsage {
            device: 1,
            usage: "Foo".to_string()
        })
    );
    assert!(matches!(
        Config::parse(&axis("X", "x * k")),
        Err(Error::Formula {
            device: 1,
            usage: X,
            ..
        })
    ));
    let twice = axis("X", "x") + "[[device.axis]]\nusage = \"X\"\ncurve = \"x\"\n";
    assert_eq!(
        Config::parse(&twice),
        Err(Error::DuplicateAxis {
            device: 1,
            usage: X
        })
    );
    assert_eq!(
        Config::parse("tolerance = -1"),
        Err(Error::InvalidTolerance(-1.0))
    );
    assert!(matches!(
        Config::parse("[[device]]\nvendor = 0x10000\nproduct = 1"),
        Err(Error::Toml(_))
    ));
    assert!(matches!(
        Config::parse("[[device]]\nvendor = 1\nproduct = 1\nserial = \"x\""),
        Err(Error::Toml(_))
    ));
}

#[test]
fn t300rs_swap_pedals_example() {
    let config = Config::parse(include_str!("../../examples/t300rs-swap-pedals.toml")).unwrap();
    let axes = &config.devices[0].axes;
    let swaps: Vec<_> = axes
        .iter()
        .map(|a| (a.usage, a.sources[0].usage, a.kind, a.sources[0].kind))
        .collect();
    let (rz, y) = (Usage::new(0x01, 0x35), Usage::new(0x01, 0x31));
    let u = AxisKind::Unipolar;
    assert_eq!(swaps, [(rz, y, u, u), (y, rz, u, u)]);
}

/// Every TOML block of the recipes is a valid configuration.
#[test]
fn recipes() {
    let recipes = include_str!("../../docs/RECIPES.md");
    let mut count = 0;
    for block in recipes.split("```toml\n").skip(1) {
        let toml = &block[..block.find("```").unwrap()];
        let config = Config::parse(toml).unwrap_or_else(|e| panic!("{e} in\n{toml}"));
        assert!(
            config.warnings.is_empty(),
            "{:?} in\n{toml}",
            config.warnings
        );
        assert!(!config.devices[0].axes.is_empty());
        count += 1;
    }
    assert_eq!(count, 9);
}

/// An empty source table leaves a formula without variables to read, so
/// it is rejected instead of reading the axis itself.
#[test]
fn empty_source_table() {
    let toml = "[[device]]\nvendor = 1\nproduct = 2\n\
                [[device.axis]]\nusage = \"X\"\ncurve = \"0\"\nsource = {}\n";
    let err = Config::parse(toml).unwrap_err();
    assert!(matches!(err, Error::EmptySource { device: 1, .. }), "{err}");
    assert!(err.to_string().contains("source table is empty"), "{err}");
}

/// Tolerances must be finite numbers of at least 0.
#[test]
fn tolerance_not_finite() {
    for t in ["inf", "-inf", "nan", "-0.5"] {
        let err = Config::parse(&format!("tolerance = {t}\n")).unwrap_err();
        assert!(matches!(err, Error::InvalidTolerance(_)), "{t}: {err}");
        assert!(
            err.to_string()
                .contains("must be a finite number of at least 0"),
            "{err}"
        );
    }
}
