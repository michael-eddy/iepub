# EPUB 解析已知问题

记录时间：2026-10-09
复现环境：Windows 10 19044 / cargo 1.97.1 / iepub 1.3.7 (master baaf9db)
样本：`docs/cole-voyage-of-life-tol.epub`、`docs/haruko-jpeg.epub`、`docs/cc-shared-culture.epub`（IDPF 官方示例；`*.epub` 在 `.gitignore` 中，样本不入库）

复现方式：临时 example 调用 `read_from_file()` 打印 chapters / nav / assets / cover / 每章正文长度，脚本未入库。
为分离变量另外构造了 7 个派生样本（构造方法见附录），下文所有数字均为实测输出。

## 一、总体表象

`read_from_file()` 对这三本书**都不返回 Err**（除 P7 的 panic 外），而是"解析成功但目录为空"。
CLI 侧表现为 `nav` 无任何输出，`get-chapter` 在漫画类书上拿到空内容。

原样样本实测：

| 项目 | cole-voyage-of-life-tol | haruko-jpeg | cc-shared-culture |
|---|---|---|---|
| OPF 位置 | `EPUB/cole.opf` | `OPS/package.opf` | `EPUB/package.opf` |
| nav 文档的 `item@id` | `nav` | `toc`（另有 `ncxtoc` 指向 NCX） | `toc` |
| `book.toc()` | None | Some("xhtml/toc.xhtml") | Some("xhtml/toc.xhtml") |
| chapters | 5，正文正常 | 13，全是 `.jpg/.png`，正文长度 0 | 8，正文正常 |
| nav | 0 条 | 0 条 | **0 条** |
| cover | images/portrait.jpg | None | images/326261902_3fa36f548d.jpg |

## 二、问题清单

### P1 目录（toc）只按硬编码 id 识别 —— 已确认，仅命中 cole

- 位置：`lib/src/epub/reader.rs:614`
  ```rust
  .find(|f| f.1.id() == "toc" || f.1.id() == "ncx")
  ```
- 规范判据：EPUB3 用 `manifest/item@properties="nav"`，EPUB2 用 `spine@toc`；`item@id` 由生成器任意取名。
- 三本比对结果：**只有 cole 触发**（它的 nav 项 id 是 `nav`，`book.toc()` 直接为 None）。
  cc-shared-culture 和 haruko 的 nav 项 id 恰好叫 `toc`，P1 不触发 —— 所以 P1 不是三本共同的 nav=0 原因，共同原因是 P2。
- 实测佐证（变体 coleA）：把 OPF 展平到 zip 根以绕开 P2，nav 项 id 仍是 `nav` → `toc()=None`、`nav=0`。说明 P1 独立致命。
- 副作用：haruko 命中 id=`toc` 拿到的是 nav.xhtml，而 `spine toc="ncxtoc"` 指向的真正 NCX 被忽略，NCX 作为普通 asset 暴露给调用方。
- 关联缺陷：`read_manifest_xml()`（`reader.rs:405` 起）读 `properties` 时只判断 `cover-image`，`nav` 值被丢弃；`read_spine_xml()` 不读 `spine@toc`。
- 修复方向：manifest 阶段记录 `properties` 含 `nav` 的 item 与 `media-type=application/x-dtbncx+xml` 的 item；spine 阶段读 `@toc`；优先级 EPUB3 nav → spine@toc 的 NCX → id 兜底。

### P2 `Path::system` 用 OS 分隔符解析 zip 内路径 —— 已确认，三本共同原因，仅 Windows

- 位置：`lib/src/path.rs:14-27`（`sep` 在 Windows 为 `\`），消费点 `lib/src/epub/reader.rs:913-930`
- 机理：zip 条目名恒为 `/`。Windows 下 `Path::system("EPUB/package.opf")` 按 `\` 切分成单段：
  - `level_count()==1` → `book.prefix` 不被设置；
  - `.pop()` 得空串 → toc 路径拼成 `xhtml/toc.xhtml`，zip 内实际条目是 `EPUB/xhtml/toc.xhtml`；
  - `reader.rs:930` 的 `if reader.by_name(t).is_ok()` 失败，**整段导航读取被静默跳过**，不报错。
- 实测佐证：`Path::system("EPUB/package.opf").pop().join("xhtml/toc.xhtml").to_str() == "xhtml/toc.xhtml"`。
- 关键对照实验：cc-shared-culture 原样 `nav=0`；仅把 OPF 展平到根目录（变体 `cc-root`，其余内容一字未改）→ `nav=6`，标题与 href 全部正确。证明 P2 是这本书目录空白的唯一原因。
- 影响：三本在 Windows 上全部中招；Linux/macOS 能拼对。
- 关联缺陷：`book.prefix`（`core.rs:726`）全项目只在 `reader.rs:917` 写入、无任何读取点，是死字段。章节/资源正文之所以能读出来，靠的是 `core.rs:172` 硬编码前缀重试表 `["", "OEBPS/", "EPUB/", "OPS/"]`（`EpubHtml::PREFIXES`）。OPF 位于其他目录（`content/`、`Text/` 等）的书会全部读不出正文。
- 修复方向：zip 内路径一律按 `/` 解析；把 OPF 所在目录前缀真正用于取数路径，取代硬编码前缀表。

### P3 固定版式书的图片被当成章节，`item@fallback` 未实现 —— 已确认，命中 haruko

- 位置：`lib/src/epub/reader.rs:343-392`（`read_spine_xml`）、`lib/src/epub/core.rs:268` 与 `:354`
- 机理：
  1. `read_spine_xml` 不校验 `media-type`，spine 引用的 item 一律 `add_chapter()`。haruko 的 spine 直接引用 12 张 jpg + 1 张 png → 13 个"章节"其实是图片本体。
  2. 取正文走 `read_string()` → `read_to_string()`，二进制 JPEG 触发 UTF-8 错误；前缀重试循环里除 `Err(IError::FileNotFound) => {}` 外是 `Err(_e) => break`，一个非预期错误会**中断整张重试表**。
  3. EPUB3 的 `item@fallback`（应回落到 `OPS/xhtml/fallback.xhtml` 取正文）在整个库中无解析点 —— `lib/src/**/*.rs` 全仓 grep `fallback` 命中 0 次。
  4. 图片同时留在 `assets` 里（`assets.remove(index)` 只在文件名含 `.xhtml` 时执行），chapters 与 assets 重复。
- 实测佐证：haruko 原样与展平变体（hC/hE/hF）都是 `chapters=13`、正文长度 0。
- 修复方向：spine 建章节时按 media-type 过滤；被跳过的图片项若带 `fallback`，挂到 fallback 指向的 xhtml；解码错误不应中断前缀重试（或按字节读再尝试 UTF-8）。

### P4 命名空间前缀未归一化 —— 已确认，命中 haruko 的 NCX

- 位置：`reader.rs:645`、`:651`、`:661`（`read_nav_point_xml`）、`:739`、`:742`、`:748`（`read_nav_xml`）、`:874`（`has_epub_type`）等所有 `e.name().as_ref() == b"ncx"` / `attr.key.0 == b"epub:type"` 形式的原始限定名比较；`read_meta_xml` 里的 `"dc:date"`、`"dc:title"` 字符串比较同理。
- 机理：quick-xml 0.41 的 `BytesStart::name()` 返回**含前缀的原始限定名**，去前缀要用同版本已提供的 `local_name()`（依赖源码 `quick-xml-0.41.0/src/events/mod.rs:226`），库未使用。
- 实测对照（唯一差异是 NCX 里的 `ncx:` 前缀）：
  - 变体 hE（走 NCX 分支，保留 `ncx:ncx`/`ncx:navMap`/`ncx:navPoint`）→ `nav=0`；
  - 变体 hF（其余完全相同，仅删掉 `ncx:` 前缀）→ `nav=1`，标题 `ハルコさんの彼氏/Haruko San No Kareshi`、href `../images/01.jpg` 都正确。
- 影响：带命名空间前缀的 NCX 一条都解析不出，`read_nav_xml` 扫到 Eof 返回空且不报错。库自身 writer 生成的 NCX 是无前缀写法（`html.rs:159`），所以现有单测覆盖不到。
- 修复方向：标签匹配统一用 `local_name()`，命名空间语义靠 `xmlns` 声明而不是前缀字面量。

### P5 EPUB3 nav 的标题丢失 —— 已确认，命中 cole 与 haruko

- 位置：`lib/src/epub/reader.rs:812-858`
- 机理：`read_nav_xhtml` 只在 `<span class="toc-label">` 的 End 事件里 `set_title()`（`:815`、`:853`）；`<a>` 的 Start 只取 href（`:796`），文本从不采集。`toc-label` 是库自身 writer 的私有格式。
- 实测：
  - 变体 coleC（nav 为标准 `<li><a href="1-childhood.xhtml#b1">Childhood</a></li>`）→ `nav=5` 条，**title 全为空串**；
  - 变体 hC → `nav=2` 条，title 全为空串；
  - 变体 cc-root（nav.xhtml 恰好带 `<span class="toc-label">`）→ `nav=6` 条标题全部正确。即 IDPF 这类带 toc-label 的书不受 P5 影响。
- 关联缺陷：href 拼接是 `format!("{}{}", root_path, href)` 的朴素拼接，不做 `..` 归一化。变体 hC 的 nav href 变成 `xhtml/../images/01.jpg`，与 chapter 的 `images/01.jpg` 对不上，`update_chapter()` 的标题匹配（`core.rs:1000`）失效。
- 修复方向：采集 `<a>` 文本作为标题（`toc-label` 优先，缺失时回退到 anchor 文本）；href 走路径归一化后再与 chapter 文件名比对。

### P6 `spine@page-progression-direction` 未解析 —— 已确认（代码级），命中 haruko

- 位置：`lib/src/epub/reader.rs:343-392`（`read_spine_xml` 只 match `Event::Empty(itemref)` 与 `Event::End(spine)`，没有 `Event::Start(spine)` 分支；`reader.rs` 全文无 direction 相关代码）
- 影响：haruko 是 `page-progression-direction="rtl"` 的漫画，方向信息在解析阶段丢失；目前方向只在单章 HTML 的 `dir`/`xml:lang` 上推断（`html.rs:475`）。
- 状态：代码级确认，未做运行时断言（`book.direction()` 未取到值，待补测试）。

### P7 nav.xhtml 含嵌套 `<ol>`（分级目录）时直接 panic —— 新发现，崩溃级

- 位置：`lib/src/epub/reader.rs:826-842`
  ```rust
  b"ol" => {
      if let Some(children) = stack.pop_back() {
          if let Some(last) = stack.back_mut() {
              for item in children {
                  last.last_mut().unwrap().push(item);   // :836 panic
  ```
- 机理：分级目录里子 `<ol>` 在父 `<li>` 之前闭合，此时父层容器 Vec 还没有任何元素（父 `<li>` 要到自己的 `</li>` 才 push），`last_mut()` 返回 None → unwrap panic。
- 实测：变体 coleB（nav.xhtml 保留原始的 `<li>…<ol><li>…</li></ol></li>` 嵌套）→
  ```
  thread 'main' panicked at lib\src\epub\reader.rs:836:49:
  called `Option::unwrap()` on a `None` value
  ```
  变体 coleC（仅去掉嵌套层）→ 不 panic。触发条件确认为"嵌套 ol"。
- 触发前提：需要先绕过 P1/P2（id 命中 + 路径拼对），所以目前被 P1/P2 掩盖。cc-shared-culture 的 nav 是平铺的所以没暴露。
- 影响面：带层级目录的 EPUB3 书非常普遍，库在解析阶段**进程级 panic**，调用方拿不到 `IResult` 错误。
- 同源风险：`reader.rs:853` 的 `current_item.as_mut().unwrap()`，`<span class="toc-label">` 出现在 `<li>` 之外时同样会 panic；`reader.rs:801` 的 `href.unwrap()` 对属性解析错误也是 unwrap。
- 另注：`Event::End` 的 `b"ol"`/`b"li"`/`b"span"` 分支没有 `in_toc_nav` 守卫（`Event::Start` 有），文档正文或其他 `<nav epub:type="landmarks">` 里的 `</ol>`/`</li>` 也会驱动栈，属于同一函数的状态机缺陷。
- 修复方向：以 `<li>` 为节点维护父子挂接（把子列表暂存到当前 li，而不是依赖"父 li 已入列"），或改用显式树构建；所有 unwrap 改为可返回 `IError` 的分支。

### P8 `itemref@linear` 与 nav 文档自身未从章节中排除 —— 已确认，命中 cc-shared-culture

- 位置：`lib/src/epub/reader.rs:343-392`
- 实测：cc-shared-culture `chapters=8`，即 spine 的 8 项全部成为章节，包含
  - `xhtml/cover.xhtml` —— 它在 spine 里是 `<itemref idref="cover" linear="no"/>`（非线性的封面页）；
  - `xhtml/toc.xhtml` —— nav 文档本身也在 spine 中，于是既作为 `book.toc()` 又作为一章。
- 机理：`read_spine_xml` 不读 `itemref@linear`；`assets.remove(index)` 的条件里显式放过了 `id=="toc"` 的项。
- 影响：调用方按 chapters 顺序渲染时，会把封面页和目录页当成正文章节。
- 修复方向：跳过 `linear="no"` 的 itemref；spine 中命中的 nav 文档不再作为章节（或标记类型让调用方自行决定）。

### P9 nav 的 `#fragment` 条目会把同一文件重复拆成章节 —— 已确认，变体实测

- 位置：`lib/src/epub/core.rs:995-1029`（`update_chapter`）
- 机理：章节文件名在 nav 中找不到同名项时，会去找"以该章节文件名开头且含 `#`"的 nav 项，把这些 nav 项作为**新章节插入**，但原有的整文件章节没有被移除或合并。
- 实测：变体 coleC `chapters` 从 5 变成 9：
  ```
  chap[1] xhtml/1-childhood.xhtml#b1  title="Childhood"  len=0
  chap[2] xhtml/1-childhood.xhtml      title="Childhood"  len=1394
  ```
  `#b1` 版本的正文长度 0（`get_html_info` 按 fragment 抽取，该文件里不存在对应 id 的元素时返回空内容）。
- 次生问题：`core.rs:1001` 的 `ele.set_title(v.title())` 是**无条件覆盖**，nav 标题为空串时会把章节原有标题清空。实测中最终标题仍正确（`"Childhood"`），是因为随后惰性读取正文时 `read_data` 用 HTML 的 `<title>` 又填了回来 —— 也就是说章节标题的正确性依赖"先读正文还是先读标题"的调用顺序，属于脆弱点而非稳定行为。
- 修复方向：fragment 章节与原文件章节去重（保留一种表述）；`set_title` 只在 nav 标题非空时覆盖。

## 三、按样本归因

- **cole-voyage-of-life-tol**：P1（nav 项 id 是 `nav`）+ P2（Windows 路径）。P1/P2 都绕开后立刻撞上 P7（嵌套 ol panic）；再退一层嵌套才有 P5（标题空）与 P9（重复章节）。正文本身可读。
- **haruko-jpeg**：P2 + P3（图片当章节、fallback 缺失）+ P4（NCX 前缀写法解析不出）+ P5（nav.xhtml 标题空、href 不规范化）+ P6（rtl 丢失）。
- **cc-shared-culture**：**不触发 P1**（id 恰好是 `toc`），nav=0 完全由 P2 造成；展平后即得 6 条正确目录。另暴露 P8（`linear="no"` 封面页与 nav 文档本身成为章节）。它的 nav 用了 `toc-label`，所以 P5 不适用；nav 平铺，所以 P7 不适用。

## 四、建议处理顺序

1. **P7** —— 唯一的崩溃项，且被 P1/P2 掩盖，修好前两个立刻会爆。应先修并补一个带嵌套 `<ol>` 的回归样本。
2. **P2** —— 三本共同、Windows 普适，改动面小（路径分隔符 + 真正使用前缀）。
3. **P1** —— 按 `properties="nav"` / `spine@toc` 定位目录。
4. **P5 + P9** —— 决定目录条目是否真的可用（有标题、不重复、能对上的章节）。
5. **P3** —— 决定固定版式/漫画类书能否读到内容。
6. **P8、P4、P6** —— 章节集合正确性、NCX 前缀兼容、阅读方向。

回归样本建议：现有三本之外，至少补一本 OPF 不在 `OEBPS/`、`EPUB/`、`OPS/` 下的书（验证 P2 的前缀表替换）、一本带层级 nav.xhtml（验证 P7/P5）、一本 EPUB2 纯 NCX 无前缀书（验证 P1 兜底与 P4）。

## 附录：派生变体构造方法

全部在临时目录构造，未入库。公共手法：解包后把 `EPUB/` 或 `OPS/` 里的内容提到 zip 根，并把 `META-INF/container.xml` 的 `full-path` 改为裸文件名，以此绕开 P2；再用 `tar.exe -a -cf x.zip ...` 重新打包（注意 `-a` 按扩展名判格式，直接输出 `.epub` 会生成 tar 而非 zip）。

| 变体 | 基于 | 相对原书的改动 | 实测结果 |
|---|---|---|---|
| cc-root | cc-shared-culture | 仅 OPF 展平 | chapters 8、nav 6（标题与 href 全对） |
| coleA | cole | 仅 OPF 展平 | `toc()=None`、nav 0 → 证明 P1 独立致命 |
| coleB | cole | 展平 + `item id="nav"` 改为 `id="toc"` | **panic at reader.rs:836** → P7 |
| coleC | coleB | 再去掉 nav.xhtml 的嵌套 `<ol>` | chapters 9、nav 5、标题全空 → P5 + P9 |
| hC | haruko | 仅 OPF 展平 | nav 2、标题全空、href `xhtml/../images/01.jpg` → P5 |
| hE | hC | `id="toc"`→`navdoc`、`id="ncxtoc"`→`ncx`（改走 NCX 分支） | nav 0 → P4 |
| hF | hE | 仅删除 `toc.ncx` 里的 `ncx:` 前缀 | nav 1、标题正确 → 反证 P4 |
