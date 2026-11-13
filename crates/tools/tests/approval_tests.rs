use super::approval::*;

#[test]
fn closures_are_approvers() {
    let yes = |_: &str| Ok(());
    let no = |c: &str| Err(format!("not {c}"));
    assert_eq!(yes.approve("ls"), Ok(()));
    assert_eq!(no.approve("ls"), Err("not ls".to_string()));
}

#[test]
fn display_indents_each_line() {
    assert_eq!(display_command("echo a\necho b"), "    echo a\n    echo b");
}

#[test]
fn display_escapes_control_chars_that_could_hide_the_command() {
    // `\r` would let `echo hi` be drawn over `cat .env` on the terminal.
    assert_eq!(display_command("cat .env\recho hi"), "    cat .env\\recho hi");
    // ANSI escape (here: clear line) is shown, not executed by the terminal.
    assert_eq!(display_command("rm x\x1b[2Kls"), "    rm x\\u{1b}[2Kls");
    // Tabs are harmless and kept.
    assert_eq!(display_command("a\tb"), "    a\tb");
}
