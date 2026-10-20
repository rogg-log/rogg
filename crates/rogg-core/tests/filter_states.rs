use rogg_core::filter::{FilterSpec, Term, TermState};
use rogg_core::settings::BehaviorSettings;

fn spec(states: &[TermState]) -> FilterSpec {
    FilterSpec { terms: states.iter().enumerate().map(|(i, s)| Term::new(format!("t{i}"), *s)).collect(), ..Default::default() }
}

#[test]
fn without_exclude_a_click_toggles_on_and_off() {
    use TermState::*;
    assert_eq!([Include.next_in(false), Off.next_in(false)], [Off, Include]);
    assert_eq!(Exclude.next_in(false), Include, "a leftover NOT turns on with the next click");
    assert_eq!([Include.next_in(true), Off.next_in(true), Exclude.next_in(true)], [Off, Exclude, Include]);
    let mut s = spec(&[Include, Off]);
    s.cycle(0, false);
    s.cycle(1, false);
    assert_eq!(s.terms.iter().map(|t| t.state).collect::<Vec<_>>(), [Off, Include]);
    s.cycle(1, true);
    assert_eq!(s.terms[1].state, Off);
    s.cycle(1, true);
    assert_eq!(s.terms[1].state, Exclude);
}

#[test]
fn excludes_become_off() {
    use TermState::*;
    let s = spec(&[Include, Exclude, Off]).without_excludes();
    assert_eq!(s.terms.iter().map(|t| t.state).collect::<Vec<_>>(), [Include, Off, Off]);
    assert_eq!(s.terms[1].text, "t1", "the term itself is kept");
}

#[test]
fn the_summary_names_exclusions_only_when_there_are_some() {
    use TermState::*;
    assert_eq!(spec(&[Include, Include, Off]).summary(), "2 on · 1 off");
    assert_eq!(spec(&[Include, Include, Exclude, Off]).summary(), "2 include · 1 exclude · 1 off");
}

#[test]
fn excluding_terms_is_off_by_default() {
    assert!(!BehaviorSettings::default().exclude_terms);
    let old: BehaviorSettings = toml::from_str("wrap = true").unwrap();
    assert!(!old.exclude_terms, "settings saved before the option load with it off");
}
