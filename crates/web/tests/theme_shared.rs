//! 教材の `pack.appearance` から IR・`Theme`・`data-theme` までの通しの試験（TASK-53 1/2、PACK-21）。

use std::fs;
use std::path::Path;

use fandhe_workshop_contract::appearance::{read_appearance, Theme};
use fandhe_workshop_pack::compile::compile;
use fandhe_workshop_pack::source;
use fandhe_workshop_web::profile::DisplaySettings;

#[test]
fn theme_flows_from_pack_to_data_theme() {
    for t in Theme::ALL {
        let dir =
            Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("theme-shared-{}", t.as_str()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("chapters/c1")).unwrap();
        fs::write(
            dir.join("workshop.yaml"),
            format!(
                "pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: A, appearance: {{theme: {}}}}}\n",
                t.as_str()
            ),
        )
        .unwrap();
        fs::write(
            dir.join("chapters/c1/chapter.yaml"),
            "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n  - {id: a, title: A, body: a.md, next: [{to: b}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-a, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}\n  - {id: b, title: B, final: true}\n",
        )
        .unwrap();
        fs::write(dir.join("chapters/c1/a.md"), "# a\n").unwrap();
        let lp = source::load(&dir).expect("load");
        let ir = compile(&lp).expect("compile");
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(read_appearance(ir.pack.appearance.as_ref()), Ok(t));
        let settings = DisplaySettings {
            theme: t,
            ..Default::default()
        };
        assert_eq!(settings.root_attrs(), [("data-theme", t.as_str())]);
    }
}
