use language_tag::Tag;
use serde::Deserialize;
use std::{borrow::Borrow, fmt::Display, iter::once, ops::Deref};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
// #[serde(default)]
pub struct TagSet {
    // Required keys
    pub full: Tag,
    #[serde(default)]
    pub iana: Vec<String>,
    pub sldr: bool,
    pub tag: Tag,
    pub windows: Tag,

    // Defaultable keys
    #[serde(default)]
    pub iso639_3: String,
    #[serde(default)]
    pub latnnames: Vec<String>,
    #[serde(default)]
    pub localname: String,
    #[serde(default)]
    pub localnames: Vec<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub nophonvars: bool,
    #[serde(default)]
    pub obsolete: bool,
    #[serde(default)]
    pub regionname: String,
    #[serde(default)]
    pub regions: Vec<String>,
    #[serde(default)]
    pub rod: String,
    #[serde(default)]
    pub suppress: bool,
    #[serde(default)]
    pub tags: Vec<Tag>,
    #[serde(default)]
    pub unwritten: bool,
    #[serde(default)]
    pub variants: Vec<String>,
}

pub trait Iter: DoubleEndedIterator + Clone {}
impl<I> Iter for I
where
    I::Item: Borrow<Tag>,
    I: DoubleEndedIterator + Clone,
{
}

pub trait SetIter: DoubleEndedIterator + Clone {}
impl<I> SetIter for I
where
    I::Item: Iter,
    <I::Item as Iterator>::Item: Borrow<Tag>,
    I: DoubleEndedIterator + Clone,
{
}

impl TagSet {
    pub fn all_tags(&self) -> impl Iter<Item = Tag> + '_ {
        self.iter()
            .cloned()
            .chain(self.region_sets().flatten())
            .chain(self.variant_sets().flatten())
    }

    pub fn iter(&self) -> impl Iter<Item = &Tag> {
        once(&self.tag)
            .chain(self.tags.iter())
            .chain(once(&self.full))
    }

    pub fn region_sets(&self) -> impl SetIter<Item = impl Iter<Item = Tag> + use<'_>> {
        self.regions.iter().map(|region| {
            self.iter().filter_map(|tag| {
                tag.region().and(Some({
                    let mut tag = tag.clone();
                    tag.set_region(region);
                    tag
                }))
            })
        })
    }

    pub fn variant_sets(&self) -> impl SetIter<Item = impl Iter<Item = Tag> + use<'_>> {
        let prototypes = once(self.iter().cloned().collect::<Vec<Tag>>())
            .chain(self.region_sets().map(Iterator::collect::<Vec<Tag>>));
        prototypes.flat_map(|prototype| {
            self.variants.iter().map(move |variant| {
                prototype.clone().into_iter().map(|mut tag| {
                    tag.push_variant(variant);
                    tag
                })
            })
        })
    }

    pub fn sldr_file_name(&self) -> Option<PathBuf> {
        if self.sldr {
            let path = self.windows.to_string().replace('-', "_") + ".xml";
            Some(path.into())
        } else {
            None
        }
    }
}

pub fn render_equivalence_set<I: IntoIterator>(set: I) -> String
where
    I::Item: Borrow<Tag>,
{
    set.into_iter()
        .map(|tag| tag.borrow().to_string())
        .reduce(|set, ref tag| set + "=" + tag)
        .unwrap()
}

impl Display for TagSet {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&render_equivalence_set(self.iter()))
    }
}

impl Deref for TagSet {
    type Target = Tag;

    fn deref(&self) -> &Self::Target {
        &self.full
    }
}

#[cfg(test)]
mod test {
    use super::TagSet;
    use language_tag::Tag;
    use serde_json::json;

    #[test]
    fn tagset() {
        let src = json!({
                        "full": "pt-Latn-BR",
                        "iana": [ "Portuguese" ],
                        "iso639_3": "por",
                        "localname": "português",
                        "localnames": [ "Português" ],
                        "name": "Portuguese",
                        "names": [ "Portugais", "Portugués", "Portugués del Uruguay", "Português", "Portunhol", "Portuñol", "Purtagaalee", "Uruguayan Portuguese" ],
                        "region": "BR",
                        "regionname": "Brazil",
                        "regions": [ "AD", "AG", "AU", "BE", "BM", "CA", "CG", "CW", "DE", "ES", "FI", "FR", "GG", "GY", "IN", "JE", "JM", "MW", "PY", "RU", "SN", "SR", "US", "UY", "VC", "VE", "ZA", "ZM" ],
                        "script": "Latn",
                        "sldr": true,
                        "suppress": true,
                        "tag": "pt",
                        "tags": [ "pt-BR", "pt-Latn" ],
                        "variants": [ "abl1943", "ai1990", "colb1945" ],
                        "windows": "pt-BR"
                    }).to_string();
        let ts: TagSet = serde_json::from_str(&src).expect("TagSet value");
        assert_eq!(
            ts,
            TagSet {
                full: Tag::builder()
                    .lang("pt")
                    .script("Latn")
                    .region("BR")
                    .build(),
                iana: vec!["Portuguese".into()],
                iso639_3: "por".into(),
                localname: "português".into(),
                localnames: vec!["Português".into()],
                name: "Portuguese".into(),
                names: vec![
                    "Portugais".into(),
                    "Portugués".into(),
                    "Portugués del Uruguay".into(),
                    "Português".into(),
                    "Portunhol".into(),
                    "Portuñol".into(),
                    "Purtagaalee".into(),
                    "Uruguayan Portuguese".into(),
                ],
                regionname: "Brazil".into(),
                regions: [
                    "AD", "AG", "AU", "BE", "BM", "CA", "CG", "CW", "DE", "ES", "FI", "FR", "GG",
                    "GY", "IN", "JE", "JM", "MW", "PY", "RU", "SN", "SR", "US", "UY", "VC", "VE",
                    "ZA", "ZM"
                ]
                .iter()
                .map(|&x| x.into())
                .collect(),
                sldr: true,
                suppress: true,
                tag: Tag::with_lang("pt"),
                tags: vec![
                    Tag::builder().lang("pt").region("BR").build(),
                    Tag::builder().lang("pt").script("Latn").build()
                ],
                variants: vec!["abl1943".into(), "ai1990".into(), "colb1945".into()],
                windows: Tag::builder().lang("pt").region("BR").build(),
                ..Default::default()
            }
        )
    }

    #[test]
    fn display_trait() {
        let test: Vec<TagSet> = serde_json::from_str(
            &json!([
                {
                    "full": "aa-Arab-ET",
                    "iana": [ "Afar" ],
                    "iso639_3": "aar",
                    "name": "Afar",
                    "nophonvars": true,
                    "region": "ET",
                    "regionname": "Ethiopia",
                    "regions": [ "DJ", "ER" ],
                    "script": "Arab",
                    "sldr": false,
                    "tag": "aa-Arab",
                    "windows": "aa-Arab-ET"
                },
                {
                    "full": "aa-Latn-ET",
                    "iana": [ "Afar" ],
                    "iso639_3": "aar",
                    "localname": "Qafar",
                    "localnames": [ "Qafar af" ],
                    "name": "Afar",
                    "region": "ET",
                    "regionname": "Ethiopia",
                    "script": "Latn",
                    "sldr": true,
                    "tag": "aa",
                    "tags": [ "aa-ET", "aa-Latn" ],
                    "windows": "aa-Latn-ET"
                }
            ])
            .to_string(),
        )
        .expect("LangTags test case.");
        let test: Vec<_> = test
            .iter()
            .map(|ts| format!("{full}: {ts}", full = ts.full))
            .collect();

        assert_eq!(
            test,
            [
                "aa-Arab-ET: aa-Arab=aa-Arab-ET",
                "aa-Latn-ET: aa=aa-ET=aa-Latn=aa-Latn-ET",
            ]
        );
    }
}
