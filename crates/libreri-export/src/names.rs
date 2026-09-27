//! Splitting "Jane Q. Smith" into family and given names for citations.

/// A person's name as citation formats need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersonName {
    Person {
        family: String,
        given: String,
    },
    /// An organisation or a single name, used as written.
    Literal(String),
}

/// Lower-case words that belong to the family name: "van", "de la"…
const PARTICLES: &[&str] = &[
    "van", "von", "de", "der", "den", "del", "della", "di", "da", "du", "la", "le", "ten", "ter",
    "dos", "das", "do", "bin", "ibn", "al", "el",
];
const SUFFIXES: &[&str] = &["Jr.", "Jr", "Sr.", "Sr", "II", "III", "IV"];

/// "Jane Q. Smith" → Smith / Jane Q.; "Ludwig van Beethoven" → van
/// Beethoven / Ludwig; "Smith, Jane" → Smith / Jane; "UNESCO" → literal.
pub fn split_name(name: &str) -> PersonName {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if let Some((family, given)) = name.split_once(',') {
        let (family, given) = (family.trim(), given.trim());
        if !family.is_empty() && !given.is_empty() && !SUFFIXES.contains(&given) {
            return PersonName::Person {
                family: family.to_owned(),
                given: given.to_owned(),
            };
        }
    }
    let mut words: Vec<&str> = name.split(' ').filter(|w| !w.is_empty()).collect();
    let suffix = match words.last() {
        Some(w) if words.len() > 2 && SUFFIXES.contains(w) => words.pop(),
        _ => None,
    };
    if words.len() < 2 {
        return PersonName::Literal(name);
    }
    let mut start = words.len() - 1;
    while start > 1 && PARTICLES.contains(&words[start - 1]) {
        start -= 1;
    }
    let mut family = words[start..].join(" ");
    if let Some(s) = suffix {
        family = format!("{family} {s}");
    }
    PersonName::Person {
        family,
        given: words[..start].join(" "),
    }
}

impl PersonName {
    pub fn family(&self) -> &str {
        match self {
            Self::Person { family, .. } => family,
            Self::Literal(s) => s,
        }
    }

    /// "Q." style initials of the given names: "Jane Q." → "J. Q.",
    /// "Jean-Paul" → "J.-P.".
    pub fn initials(&self) -> String {
        let Self::Person { given, .. } = self else {
            return String::new();
        };
        given
            .split(' ')
            .filter(|w| !w.is_empty())
            .map(|w| {
                w.split('-')
                    .filter_map(|p| p.chars().next())
                    .map(|c| format!("{}.", c.to_uppercase()))
                    .collect::<Vec<_>>()
                    .join("-")
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// "Smith, Jane Q." (or the literal name).
    pub fn inverted(&self) -> String {
        match self {
            Self::Person { family, given } => format!("{family}, {given}"),
            Self::Literal(s) => s.clone(),
        }
    }

    /// "Jane Q. Smith".
    pub fn natural(&self) -> String {
        match self {
            Self::Person { family, given } => format!("{given} {family}"),
            Self::Literal(s) => s.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(family: &str, given: &str) -> PersonName {
        PersonName::Person {
            family: family.into(),
            given: given.into(),
        }
    }

    #[test]
    fn splits_common_names() {
        assert_eq!(split_name("Jane Q. Smith"), p("Smith", "Jane Q."));
        assert_eq!(
            split_name("Ludwig van Beethoven"),
            p("van Beethoven", "Ludwig")
        );
        assert_eq!(split_name("Smith, Jane"), p("Smith", "Jane"));
        assert_eq!(
            split_name("Martin Luther King Jr."),
            p("King Jr.", "Martin Luther")
        );
        assert_eq!(split_name("UNESCO"), PersonName::Literal("UNESCO".into()));
        assert_eq!(split_name("Jean-Paul Sartre").initials(), "J.-P.");
        assert_eq!(split_name("Jane Q. Smith").initials(), "J. Q.");
    }
}
