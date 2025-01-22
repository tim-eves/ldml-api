use crate::{tagset::TagSet, StringRepr};
use language_tag::{ExtensionRef, Tag};
use std::collections::{HashMap as Map, HashSet as Set};

#[derive(Debug, Default, PartialEq)]
pub struct LangTags {
    pub(crate) scripts: Set<StringRepr>,
    pub(crate) regions: Set<StringRepr>,
    pub(crate) variants: Set<StringRepr>,
    pub(crate) latn_variants: Set<StringRepr>,
    pub(crate) tagsets: Vec<TagSet>,
    pub(crate) full: Map<StringRepr, u32>,
}

impl LangTags {
    pub(crate) fn build_caches(&mut self) {
        for (i, ts) in self.tagsets.iter().enumerate() {
            self.full
                .extend(ts.iter().map(|tag| (tag.as_ref().into(), i as u32)));
            self.scripts.insert(ts.script().unwrap().into());
            self.regions.insert(ts.region().unwrap().into());
            self.regions.extend(ts.regions.iter().cloned());
        }
    }

    pub(crate) fn shrink_to_fit(&mut self) {
        self.scripts.shrink_to_fit();
        self.regions.shrink_to_fit();
        self.variants.shrink_to_fit();
        self.latn_variants.shrink_to_fit();
        self.tagsets.shrink_to_fit();
    }

    pub fn conformant(&self, tag: &Tag) -> bool {
        let valid_script = tag
            .script()
            .map(|s| self.scripts.contains(s))
            .unwrap_or(true);
        let valid_region = tag
            .region()
            .map(|s| self.regions.contains(s))
            .unwrap_or(true);
        valid_script && valid_region
    }

    fn valid_region(ts: &TagSet, region: Option<&str>) -> bool {
        if let Some(region) = region {
            ts.region() == Some(region) || ts.regions.contains(&region.into())
        } else {
            true
        }
    }

    fn valid_variants(&self, ts: &TagSet, tag: &Tag) -> bool {
        !tag.has_variants()
            || tag.variants().all(|v| {
                let v = v.into();
                ts.variants.contains(&v)
                    || self.variants.contains(&v)
                    || (!ts.nophonvars
                        && (ts.tag.script().is_none() || ts.script() == Some("Latn"))
                        && self.latn_variants.contains(&v))
            })
    }

    fn valid_extensions<'a>(
        ts: &TagSet,
        extensions: impl IntoIterator<Item = ExtensionRef<'a>>,
    ) -> bool {
        let supplied_extensions: Set<_> = extensions.into_iter().collect();
        supplied_extensions.is_empty() || {
            let mut candidate_extensions = ts.iter().map(|t| t.extensions().collect::<Set<_>>());
            candidate_extensions.any(|tc| !tc.is_subset(&supplied_extensions))
        }
    }

    pub fn orthographic_normal_form(&self, tag: &Tag) -> Option<&TagSet> {
        let mut key = tag.clone();
        let idx = self
            .full
            .get(key.as_ref())
            .or_else(|| {
                key.set_private("");
                self.full.get(key.as_ref())
            })
            .or_else(|| {
                key.set_extensions([]);
                self.full.get(key.as_ref())
            })
            .or_else(|| {
                key.set_variants([]);
                self.full.get(key.as_ref())
            })
            .or_else(|| {
                key.set_region("");
                self.full.get(key.as_ref())
            });
        let idx = *idx? as usize;
        let ts = self.tagsets.get(idx)?;

        let private_is_valid = ts.full.private().map_or(true, |ts_priv| {
            tag.private().is_some_and(|tag_priv| tag_priv == ts_priv)
        });
        if key == *tag
            || LangTags::valid_region(ts, tag.region())
                && self.valid_variants(ts, tag)
                && LangTags::valid_extensions(ts, tag.extensions())
                && private_is_valid
        {
            Some(ts)
        } else {
            None
        }
    }

    pub fn locale_normal_form(&self, tag: &Tag) -> Option<TagSet> {
        self.orthographic_normal_form(tag).map(|ortho_tagset| {
            let mut ts = ortho_tagset.clone();
            if let Some(region) = tag.region() {
                let ri = ts.regions.iter().position(|x| x == region).unwrap();
                ts.regions[ri] = ts.region().unwrap().into();
                ts.full.set_region(region);
                ts.tag.set_region(region);
                for i in (0..ts.tags.len()).rev() {
                    if ts.tags[i].region().is_none() {
                        ts.tags.remove(i);
                    }
                }
                for t in &mut ts.tags {
                    t.set_region(region);
                }
            }
            ts
        })
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&Tag, &TagSet)> + Clone {
        self.tagsets
            .iter()
            .flat_map(|ts| ts.iter().map(move |t| (t, ts)))
    }

    pub fn tagsets(&self) -> impl DoubleEndedIterator<Item = &TagSet> + Clone {
        self.tagsets.iter()
    }

    #[inline(always)]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.tagsets.len()
    }
}

#[cfg(test)]
mod test {}
