/// 章节信息，html片段
#[derive(Debug)]
pub struct MobiHtml {
    title: String,
    /// 原始数据，经编解码后方可阅读
    raw: Option<Vec<u8>>,
    /// 在整个文本中的索引位置，读取时使用
    index: usize,
    /// 唯一id，写入时需要 确保nav能正确指向章节，否则目录会错误
    pub(crate) id: usize,
    /// 可阅读的文本
    data: String,

    pub(crate) nav_id: usize,
}

impl MobiHtml {
    pub fn new(id: usize) -> Self {
        Self {
            title: String::new(),
            raw: None,
            index: 0,
            id,
            data: String::new(),
            nav_id: 0,
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn data(&self) -> &str {
        &self.data
    }

    pub fn nav_id(&self) -> usize {
        self.nav_id
    }

    pub fn with_data(mut self, value: &str) -> Self {
        self.data = value.to_string();
        self
    }

    pub fn with_title(mut self, value: &str) -> Self {
        self.title = value.to_string();
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MobiNav {
    /// id，唯一
    pub(crate) id: usize,
    pub(crate) title: String,
    pub(crate) href: usize,
    pub(crate) children: Vec<MobiNav>,
    /// 写入时指向章节
    pub(crate) chap_id: usize,
}

impl MobiNav {
    pub fn title(&self) -> &str {
        &self.title
    }
    /// 读取时使用该方法
    pub fn default(id: usize) -> Self {
        Self {
            id,
            title: Default::default(),
            href: Default::default(),
            children: Default::default(),
            chap_id: 0,
        }
    }

    /// 写入时使用该方法
    pub fn new(id: usize, chap_id: usize) -> Self {
        Self {
            id,
            title: Default::default(),
            href: Default::default(),
            children: Default::default(),
            chap_id,
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn children(&self) -> std::slice::Iter<MobiNav> {
        self.children.iter()
    }

    pub fn with_chap_id(mut self, chap_id: usize) -> Self {
        self.chap_id = chap_id;
        self
    }

    pub fn with_title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    pub fn add_child(&mut self, child: MobiNav) {
        self.children.push(child);
    }
}

/// 由于目录存在嵌套，所以需要拿到最底层的那级目录，这样才能准确的拆分文本
///
fn flatten_nav(nav: &[MobiNav]) -> Vec<&MobiNav> {
    let mut n = Vec::new();
    for ele in nav {
        if ele.children.is_empty() {
            n.push(ele);
        } else {
            n.append(&mut flatten_nav(&ele.children));
        }
    }
    n
}

/// 给nav设置对应的章节chap_id
fn set_nav_id(nav: &mut [MobiNav], nav_id: usize, chap_id: usize) -> bool {
    for ele in nav {
        if ele.id == nav_id {
            ele.chap_id = chap_id;
            return true;
        }

        if set_nav_id(&mut ele.children, nav_id, chap_id) {
            // 下级找到，当前也要设置
            ele.chap_id = chap_id;
            return true;
        }
    }
    false
}

#[derive(Debug)]
pub struct MobiAssets {
    pub(crate) _file_name: String,
    pub(crate) media_type: String,
    pub(crate) _data: Option<Vec<u8>>,
    pub(crate) recindex: usize,
}

impl MobiAssets {
    pub fn new(data: Vec<u8>) -> Self {
        MobiAssets {
            _file_name: String::new(),
            media_type: String::new(),
            _data: Some(data),
            recindex: 0,
        }
    }
    pub fn data(&self) -> Option<&[u8]> {
        self._data.as_ref().map(|f| f.as_slice())
    }
    pub fn file_name(&self) -> &str {
        &self._file_name
    }

    pub fn with_file_name(mut self, file_name: &str) -> Self {
        self._file_name = file_name.to_string();
        self
    }
}

#[derive(Debug, Default)]
pub struct MobiBook {
    info: crate::common::BookInfo,
    /// 上次修改时间
    last_modify: Option<String>,
    /// 电子书创建者信息
    generator: Option<String>,
    /// 章节
    chapters: Vec<MobiHtml>,
    /// 封面
    cover: Option<MobiAssets>,
    /// 所有图片
    images: Vec<MobiAssets>,
    /// 目录
    nav: Option<Vec<MobiNav>>,
}

impl MobiBook {
    iepub_derive::option_string_method!(info, creator);
    iepub_derive::option_string_method!(info, description);
    iepub_derive::option_string_method!(info, contributor);
    iepub_derive::option_string_method!(info, date);
    iepub_derive::option_string_method!(info, format);
    iepub_derive::option_string_method!(info, publisher);
    iepub_derive::option_string_method!(info, subject);
    // /
    // / 设置epub最后修改时间
    // /
    // / # Examples
    // /
    // / ```
    // / let mut epub = EpubBook::default();
    // / epub.set_last_modify("2024-06-28T08:07:07UTC");
    // / ```
    // /
    iepub_derive::option_string_method!(last_modify);
    iepub_derive::option_string_method!(generator);

    pub fn set_title(&mut self, title: &str) {
        self.info.title.clear();
        self.info.title.push_str(title);
    }
    pub fn title(&self) -> &str {
        self.info.title.as_str()
    }
    pub fn with_title(mut self, title: &str) -> Self {
        self.set_title(title);
        self
    }
    pub fn identifier(&self) -> &str {
        self.info.identifier.as_str()
    }
    pub fn set_identifier(&mut self, identifier: &str) {
        self.info.identifier.clear();
        self.info.identifier.push_str(identifier);
    }
    pub fn with_identifier(mut self, identifier: &str) -> Self {
        self.set_identifier(identifier);
        self
    }

    pub fn set_cover(&mut self, cover: MobiAssets) {
        self.cover = Some(cover);
    }

    pub fn cover(&self) -> Option<&MobiAssets> {
        self.cover.as_ref()
    }

    pub fn cover_mut(&mut self) -> Option<&mut MobiAssets> {
        self.cover.as_mut()
    }
    pub fn assets_mut(&mut self) -> std::slice::IterMut<MobiAssets> {
        self.images.iter_mut()
    }

    pub fn assets(&self) -> std::slice::Iter<MobiAssets> {
        self.images.iter()
    }

    pub fn add_assets(&mut self, asset: MobiAssets) {
        self.images.push(asset)
    }

    pub fn chapters_mut(&mut self) -> std::slice::IterMut<MobiHtml> {
        self.chapters.iter_mut()
    }

    pub fn chapters(&self) -> std::slice::Iter<MobiHtml> {
        self.chapters.iter()
    }

    pub fn add_chapter(&mut self, chap: MobiHtml) {
        self.chapters.push(chap);
    }

    pub fn nav(&self) -> Option<std::slice::Iter<MobiNav>> {
        self.nav.as_ref().map(|f| f.iter())
    }

    pub fn add_nav(&mut self, value: MobiNav) {
        if let Some(nav) = &mut self.nav {
            nav.push(value);
        } else {
            self.nav = Some(vec![value]);
        }
    }
}

use std::{
    io::{Read, Seek},
    sync::atomic::AtomicUsize,
};

use serde::Serialize;

use crate::common::IResult;

use super::{common::do_time_format, reader::MobiReader};

impl<T: Read + Seek> MobiReader<T> {
    pub fn load(&mut self) -> IResult<MobiBook> {
        let meta = self.read_meta_data()?;

        let mut chapters = Vec::new();
        let sec = self.load_text()?;

        let mut nav = self.read_nav_from_text(&sec[..])?;

        // 根据目录拆分文本
        let id: AtomicUsize = AtomicUsize::new(0);
        if let Some(n) = &nav {
            chapters.append(
                &mut flatten_nav(n)
                    .iter()
                    .map(|f| (sec.iter().find(|m| m.end > f.href), f))
                    .filter(|s| s.0.is_some())
                    .map(|f| (f.0.unwrap(), f.1))
                    .map(|(sec, nav)| MobiHtml {
                        id: id.fetch_add(1, std::sync::atomic::Ordering::Release),
                        nav_id: nav.id,
                        index: sec.index,
                        title: nav.title.clone(),
                        raw: None,
                        data: sec.data.clone(),
                    })
                    .collect(),
            );
            // 将 chap_id 赋予 nav
            if let Some(nav) = &mut nav {
                for ele in &chapters {
                    let _ = set_nav_id(nav, ele.nav_id, ele.id);
                }
            }
        } else if cfg!(feature = "no_nav") {
            for (index, s) in sec.iter().enumerate() {
                chapters.push(MobiHtml {
                    id: id.fetch_add(1, std::sync::atomic::Ordering::Release),
                    nav_id: index,
                    index: s.index,
                    title: format!("{}", index + 1),
                    raw: None,
                    data: s.data.clone(),
                });
            }
        } else {
            return Err(crate::common::IError::NoNav(
                r#"book has no nav, enable feature "no_nav" to generate nav"#,
            ));
        }

        let cover = self.read_cover()?;

        let c = meta.contributor.clone();

        Ok(MobiBook {
            info: meta,
            last_modify: Some(do_time_format(self.pdb_header.modify_date)),
            generator: c,
            chapters,
            cover: cover.map(|f| MobiAssets {
                _file_name: f.get_file_name(),
                media_type: String::new(),
                _data: Some(f.0),
                recindex: 0,
            }),
            images: self.read_all_image()?,
            nav,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use crate::{common::IError, mobi::reader::MobiReader};

    fn download_file(name: &str, url: &str) {
        if std::fs::metadata(name).is_err() {
            // 下载并解压

            let mut zip = tinyget::get(url)
                .send()
                .map(|v| v.as_bytes().to_vec())
                .map_err(|e| IError::InvalidArchive("download fail"))
                .and_then(|f| {
                    zip::ZipArchive::new(std::io::Cursor::new(f))
                        .map_err(|e| IError::InvalidArchive("download fail"))
                })
                .unwrap();
            let mut zip = zip.by_name(name).unwrap();
            let mut v = Vec::new();
            zip.read_to_end(&mut v).unwrap();
            std::fs::write(name, &mut v).unwrap();
        }
    }

    #[test]
    fn test_load() {
        let name = "3252.mobi";
        download_file(
            name,
            "https://github.com/user-attachments/files/18904584/3252.zip",
        );

        let path = std::env::current_dir().unwrap().join(name);
        let mut mobi =
            MobiReader::new(std::fs::File::open(path.to_str().unwrap()).unwrap()).unwrap();

        let book = mobi.load().unwrap();

        println!("{:?}", book.info);

        println!("======");

        assert_eq!(24, book.chapters.len());
        println!("{:?}", book.chapters[0]);

        println!("======");

        println!("{:?}", book.chapters[20]);
        println!("======");

        assert_eq!(1, book.images.len());
        println!("======");
        for ele in &book.chapters {
            println!("{} {}", ele.title, ele.nav_id);
        }
        println!("======");
        println!("{:?}", book.nav);
    }

    #[test]
    #[cfg(feature = "no_nav")]
    fn test_no_nav() {
        let name = "convert.mobi";
        download_file(
            name,
            "https://github.com/user-attachments/files/18818424/convert.mobi.zip",
        );

        let path = std::env::current_dir().unwrap().join(name);
        let mut mobi =
            MobiReader::new(std::fs::File::open(path.to_str().unwrap()).unwrap()).unwrap();

        let book = mobi.load().unwrap();

        println!("{:?}", book.info);

        assert_eq!(188, book.chapters.len());
    }

    #[test]
    #[cfg(not(feature = "no_nav"))]
    #[should_panic(
        expected = r#"called `Result::unwrap()` on an `Err` value: NoNav("book has no nav, enable feature \"no_nav\" to generate nav")"#
    )]
    fn test_no_nav() {
        let name = "convert.mobi";
        download_file(
            name,
            "https://github.com/user-attachments/files/18818424/convert.mobi.zip",
        );

        let path = std::env::current_dir().unwrap().join(name);
        let mut mobi =
            MobiReader::new(std::fs::File::open(path.to_str().unwrap()).unwrap()).unwrap();

        let book = mobi.load().unwrap();

        println!("{:?}", book.info);

        assert_eq!(188, book.chapters.len());
    }
}
