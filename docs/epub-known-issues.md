# EPUB 解析已知问题

记录时间：2026-10-09
复现环境：Windows 10 19044 / rustc 1.97.1 / iepub 1.3.7 (master baaf9db)
样本：`test/cole-voyage-of-life-tol.epub`、`test/haruko-jpeg.epub`（IDPF 官方示例，固定版式类；`*.epub` 在 `.gitignore` 中，样本不入库）

## 一、总体表象

`read_from_file()` 对这两本书**都不返回 Err**，而是"解析成功但内容为空"。
CLI 侧表现为 `nav` 无任何输出、`get-chapter` 拿到空内容。

实测结果（临时 example 调用 `read_from_file` 打印，已删除）：

| 项目 | cole-voyage-of-life-tol.epub | haruko-jpeg.epub |
|---|---|---|
| 元数据 title / identifier / version | 正常 | 正常 |
| chapters | 5，正文可读（`parser()` 有值） | 13，全部是 `.jpg` / `.png`，`data` 长度 0，`parser()` 返回 None |
| nav | 0 条 | 0 条 |
| cover | `images/portrait.jpg` | None |
| `book.toc()` | None | Some("xhtml/toc.xhtml") |

## 二、问题清单

### P1 目录（toc）只按硬编码 id 识别 —— 已确认

- 位置：`lib/src/epub/reader.rs:614`
  ```rust
  .find(|f| f.1.id() == "toc" || f.1.id() == "ncx")
  ```
- 规范判据：EPUB3 用 `manifest/item@properties="nav"`，EPUB2 用 `spine@toc`，`item@id` 由生成器任意取名。
- 影响：
  - cole 的 nav 项 id 是 `nav` → 完全定位不到 toc → nav=0，**与平台无关**。
  - haruko 侥幸命中 id=`toc`（指向 `xhtml/toc.xhtml`），而 `spine toc="ncxtoc"` 指向的真正 NCX 被忽略，NCX 反而作为普通 asset 暴露给调用方。
- 关联缺陷：`read_manifest_xml()`（`reader.rs:405` 起）解析 `properties` 属性时只判断了 `cover-image`，`nav` 值被直接丢弃；`read_spine_xml()` 不读 `spine@toc`。
- 修复方向：manifest 阶段记录 `properties` 含 `nav` 的 item 与 `media-type == application/x-dtbncx+xml` 的 item；spine 阶段读 `@toc`；优先级 nav.xhtml(EPUB3) → spine@toc 指向的 NCX → 现有的 id 兜底。

### P2 `Path::system` 用 OS 分隔符解析 zip 内路径 —— 已确认（仅 Windows）

- 位置：`lib/src/path.rs:14-27`（`sep` 在 Windows 上是 `\`），消费点 `lib/src/epub/reader.rs:913-930`
- 机理：zip 条目名恒为 `/`。Windows 下 `Path::system("OPS/package.opf")` 按 `\` 切分得到单段：
  - `level_count() == 1` → `book.prefix` 不被设置；
  - `.pop()` 得到空串 → 拼出的 toc 路径为 `xhtml/toc.xhtml`，zip 内实际条目是 `OPS/xhtml/toc.xhtml`；
  - `reader.rs:930` 的 `if reader.by_name(t).is_ok()` 匹配失败，**整段导航读取被静默跳过**，不报错。
- 实测佐证：`Path::system("OPS/package.opf").pop().join("xhtml/toc.xhtml").to_str() == "xhtml/toc.xhtml"`。
- 影响：haruko 的 nav=0 只在 Windows 复现；Linux/macOS 能拼对。
- 关联缺陷：`book.prefix`（`core.rs:726`）全项目只在 `reader.rs:917` 写入、无任何读取点，是死字段。章节/资源正文之所以还能读出来，靠的是 `core.rs:172` 硬编码的前缀重试表 `["", "OEBPS/", "EPUB/", "OPS/"]`（`EpubHtml::PREFIXES`）。OPF 位于其他目录（如 `content/`、`Text/`）的书会全部读不出正文。
- 修复方向：zip 内路径一律用 `/` 解析（新增 `Path::unix()` 或让 `Path::system` 同时接受 `/`）；把 OPF 所在目录前缀真正落到取数路径上，取代硬编码前缀表。

### P3 固定版式书的图片被当成章节，`item@fallback` 未实现 —— 已确认

- 位置：`lib/src/epub/reader.rs:343-392`（`read_spine_xml`）、`lib/src/epub/core.rs:268` 与 `:354`
- 机理：
  1. `read_spine_xml` 对 spine 引用的 item 不校验 `media-type`，一律 `add_chapter()`。haruko 的 spine 直接引用 12 张 jpg + 1 张 png，于是 13 个"章节"其实是图片本体。
  2. 取正文走 `read_string()` → `read_to_string()`，二进制 JPEG 触发 UTF-8 解码错误；`core.rs` 前缀重试循环里 `Err(IError::FileNotFound) => {}` 之外的分支是 `Err(_e) => break`，一个非预期错误会**中断整张前缀重试表**。
  3. EPUB3 的 `item@fallback`（本应回落到 `OPS/xhtml/fallback.xhtml` 取文本）在整个库中没有任何解析点 —— 全仓 grep `fallback` 在 `lib/src/**/*.rs` 命中 0 次。
  4. 这些图片同时留在 `assets` 里（`assets.remove(index)` 只在 `file_name().contains(".xhtml")` 时执行），chapters 与 assets 重复。
- 修复方向：spine 建章节时按 `media-type` 过滤（只收 `application/xhtml+xml`、`text/html`）；被跳过的图片项若带 `fallback`，改为挂到 fallback 指向的 xhtml；`read_data` 的解码错误分支不应 break 整张前缀表（或先按字节读取再尝试 UTF-8）。

### P4 命名空间前缀未归一化 —— 已确认（代码级）

- 位置：`reader.rs:645`、`:651`、`:661`（`read_nav_point_xml`）、`:739`、`:742`、`:748`（`read_nav_xml`）、`:874`（`has_epub_type`）等所有 `e.name().as_ref() == b"ncx"` / `attr.key.0 == b"epub:type"` 形式的原始限定名比较；`read_meta_xml` 里的 `"dc:date"`、`"dc:title"` 等字符串比较同理。
- 机理：quick-xml 0.41 的 `BytesStart::name()` 返回**原始限定名**（含前缀），去前缀要用同版本已提供的 `local_name()`（依赖源码 `quick-xml-0.41.0/src/events/mod.rs:226`），库未使用。
- 影响：haruko 的 `toc.ncx` 通篇是 `ncx:ncx` / `ncx:navMap` / `ncx:navPoint` 写法，`read_nav_xml` 一条都匹配不上，parent 栈进不去，扫到 Eof 返回空 nav 且无错误。即 P1/P2 修完后这类书仍然没有目录。
- 说明：库自身 writer 生成的 NCX 用的是无前缀写法（`html.rs:159`），所以现有单测覆盖不到这个分支。
- 修复方向：标签匹配统一改用 `local_name()`，命名空间语义靠 `xmlns` 声明而非前缀字面量。

### P5 EPUB3 nav 的标题丢失 —— 已确认（代码级）

- 位置：`lib/src/epub/reader.rs:812-858`
- 机理：`read_nav_xhtml` 只在 `<span class="toc-label">` 的 End 事件里 `set_title()`；`<a>` 的 Start 只取 `href`，文本从不采集（`b"a"` 分支在 `:796`）。`toc-label` 是库自身 writer 的私有格式。
- 影响：cole 的 `nav.xhtml` 是标准写法 `<li><a href="1-childhood.xhtml#b1">Childhood</a></li>`，即使 P1/P2 修好，nav 条目 title 仍是空串，`update_chapter()` 也就给章节填不出标题。
- 关联缺陷：href 拼接是 `format!("{}{}", root_path, href)` 的朴素字符串拼接，不做 `..` 归一化。haruko 的 `../images/01.jpg` 会变成 `xhtml/../images/01.jpg`，与 chapter 的 `images/01.jpg` 对不上，`update_chapter` 的标题匹配（`core.rs:1000`）也会失效。

### P6 `spine@page-progression-direction` 未解析 —— 已确认（代码级）

- 位置：`lib/src/epub/reader.rs:343-392`（`read_spine_xml` 只 match `Event::Empty(itemref)` 与 `Event::End(spine)`，没有 `Event::Start(spine)` 分支；`reader.rs` 全文无 direction 相关代码）
- 影响：haruko 是 `page-progression-direction="rtl"` 的漫画，阅读方向信息在解析阶段直接丢失。当前方向只在单章 HTML 的 `dir`/`xml:lang` 上推断（`html.rs:475`）。

## 三、按样本归因

- cole-voyage-of-life-tol：P1（目录定位不到）→ P5（修好 P1 后标题仍为空）。正文可正常读出，属于 P2 里硬编码前缀表 `EPUB/` 恰好命中。
- haruko-jpeg：P2（Windows 下 toc 路径拼错，静默跳过）+ P3（13 个图片章节无正文、fallback 未实现）+ P4（NCX 前缀写法匹配不上）+ P5 + P6。

## 四、建议处理顺序

1. P1 + P2 —— "目录出不来"的直接原因，改动面最小。
2. P3 —— 决定固定版式/漫画类书能否读到内容。
3. P4 + P5 —— 决定 P1/P2 修完后目录条目是否真的可用（有没有标题、能不能对上章节）。
4. P6 —— 渲染正确性。

回归样本建议保留这两本，另加一本 OPF 不在 `OEBPS/`、`EPUB/`、`OPS/` 三个目录下的书，用于验证 P2 里硬编码前缀表的替换。
