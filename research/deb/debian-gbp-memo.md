---
title: "Git buildpackageでDebian Packageを扱うメモ"
emoji: "📝"
type: "tech" # tech: 技術記事 / idea: アイデア
topics: [debian,git,zennfes2025free]
published: true
---
## 要約

`gbp pq` コマンドを使いこなそう。

`gbp pq`コマンドは、patch queue (upstreamのソースコードに対するパッチ集でdebian/patches/に存在するファイル群)をGitのコミットから生成するためのコマンドです。

## はじめに

Debianパッケージを作ってメンテナンスするのにGitを利用するgit buildpackageを使うことは知っている人いるでしょう。
参考文献にあるように、さまざまな人がgit buildpackageについて書いています。が、最近までちゃんと理解しておらずとても困った状態に陥ってしまったので、どういうワークフローでパッケージメンテナンスをすると幸せになるのか。

というのが、本記事の内容です。

git buildpackageは、以下gbpと表記します。コマンド名もgbpですが、実例時に出てくるだけなので混乱しないでしょう。

gbpは、さまざまな背景を持つDebianパッケージに対応するため、設定でメンテナンス上の要求を満たすようになっています。

### この記事はだれ向けか

自分が記事を書いて、情報をまとめておくと、将来の自分が助かるということで、自分向けです。

### この記事の読み方

最初は関係するメモ書きにします。そのためレベル感の揃わない雑多なメモ集になります。
一定以上の情報が集まって構造化までいくとよいのですが。

## gbpを使うにあたっての前提知識

<!-- textlint-disable textlint-rule-no-doubled-joshi  -->
gbpを使う前にDebian packageの前提知識としてDebianのソースコードに対する考え方、パッケージメンテナーが注意する2つの問題があります。
<!-- textlint-disable textlint-rule-no-doubled-joshi  -->

Debian Projectでは、既存のソフトウェアをDebパッケージにしてDebian Packageを作っています。
基本は、Upstreamがリリースしたソフトウェア一式のtarballを使うことが多いです。

Debian Packageではパッケージ化をおこなう際に、厳密にupstreamのリリースした内容と、Debianパッケージを作るに当たって
upstreamのソースコードを改変するパッチ集を分離します。パッチを監査して、upstreamとパッケージメンテナの責任を分離
するためです。

そのパッチ管理システムがquiltです。"debian/source/format"が下記のようになっているのをdebian packageをひもといて
いるとわかります。
```
cat debian/source/format 
3.0 (quilt)
```

個々のパッチについては、[DEP-3: Patch Tagging Guidelines](https://dep-team.pages.debian.net/deps/dep3/)
というガイドラインがあります。`quilt`や`dpkg-source --commit`でパッチをつくる時は意識していました。

`gbp pq import`して、patch-queue/作業ブランチでコミットを作るときにプログラムが自動的につけるわけ
ではないので同様の情報を残したくなるでしょう。


ただしgbpを使うなら生の`quilt`や`dquilt`または、`dpkg-source --commit`を使うのは自分で自分の足を撃ち抜いても直せる人だけがわかって使うのならいいけど、基本は「混ぜるな危険」です。
今回xfireworksで苦労したのも`gbp pq`を理解せずに、`dpkg-source --commit`を使ったのが原因でした。

### gbpで知るべきブランチ

- pristine-tarブランチ
- upstreamブランチ
- パッケージを作るために作業するブランチ。
    - debianの開発版だけに作業するなら、mainないしはmasterを指定する。
    - 複数のディストリビューションや、Debianだけでも開発版以外にbackportsへパッケージを提供する場合などはブランチを使い分ける。

以上3つのブランチに加えて、`gbp pq import`で作られ`gbp pq export`にて最終的には消滅させる"patch-queue/作業ブランチ"の合計4つ
のブランチを意識する必要がある。

実例としては下記です。この状態では`gbp pq import`コマンドはpatch-queueブランチを作っていない。

- 作業ブランチがmasterブランチ
- upstreamのtarballから展開されたソースコードを保持しているupstramブランチ

pristine-tarが、upstreamのtarballの情報を新しいリリースがでる毎にデルタと呼ばれる差分管理を保持するブランチがpristine-tarブランチです。
pristine-tarの扱いでdeltaの管理もパッケージ更新時に思い出す必要があります。
```
git branch -a
* master                                             pristine-tar
remotes/origin/HEAD -> origin/master                 remotes/origin/pristine-tar
remotes/origin/master                                upstream
remotes/origin/upstream
```

```
git switch pristine-tar 
Switched to branch 'pristine-tar'
Your branch is up to date with 'origin/pristine-tar'.

ls -la
合計 16
drwxrwxr-x 1 yabuki yabuki  130  9月 18 00:14 .
drwxr-xr-x 1 yabuki yabuki 1934  9月 15 06:34 ..
drwxrwxr-x 1 yabuki yabuki  220  9月 18 00:14 .git
-rw-rw-r-- 1 yabuki yabuki 2032  9月 18 00:14 xfireworks_1.3.orig.tar.gz.delta
-rw-rw-r-- 1 yabuki yabuki   41  9月 18 00:14 xfireworks_1.3.orig.tar.gz.id
```

```
git switch upstream 
Switched to branch 'upstream'
Your branch is up to date with 'origin/upstream'.

ls
AUTHORS        COPYRIGHT      ColorGC.c   DispP.h   OMAKE.jpn  Piece.h   StreamP.h      arguments.h  mkconf.c
AfterImage.c   Calculator.c   ColorGC.h   HISTORY   Obj.c      PieceP.h  XFireworks.c   configure.h  xfireworks.1
AfterImage.h   Calculator.h   ColorGCP.h  INSTALL   Obj.h      README    XFireworks.h   etc.c        xfireworks.conf
AfterImageP.h  CalculatorP.h  Disp.c      Makefile  ObjP.h     Stream.c  XFireworksP.h  etc.h
COPYING        ChangeLog      Disp.h      NEWS      Piece.c    Stream.h  arguments.c    main.c
```


### formatが3.0 (quilt)の形式って何よ?

gbpはquiltを使わずにdebianディレトクリ配下の"patches/"を操作します。内容は下記のような感じです。
```
ls -la debian/patches/
合計 40
drwxrwxr-x 1 yabuki yabuki  424  9月 16 23:04 .
drwxrwxr-x 1 yabuki yabuki  212  9月 16 23:04 ..
-rw-rw-r-- 1 yabuki yabuki 6270  9月 16 23:04 0007-Fix-1114447-FTBFS.patch
-rw-rw-r-- 1 yabuki yabuki  622  9月 16 23:04 0008-Update-FSF-address.patch
-rw-rw-r-- 1 yabuki yabuki  954  9月 16 23:04 0008-blhc-tells-me-need-more-hardening.patch
-rw-rw-r-- 1 yabuki yabuki  607  9月 16 23:04 Fix-FBTFS-for-mkconf
-rw-rw-r-- 1 yabuki yabuki  933  9月 16 23:04 Fix-FBTFS-for-time_t-transition
-rw-rw-r-- 1 yabuki yabuki 1064  9月 16 23:04 cross-build-dh_auto_build
-rw-rw-r-- 1 yabuki yabuki 2368  9月 16 23:04 debian-changes-1.3-7
-rw-rw-r-- 1 yabuki yabuki 2558  9月 16 23:04 hardening
-rw-rw-r-- 1 yabuki yabuki  214  9月 16 23:04 series
```
xfireworksは途中から、gbpに移行したので、最初に連番がついてないやつとついているやつが混在しています。gbpを使うと最初に連番と
patch-queue/masterなどのパッチキューにて作業したコミットです。パッチキューのファイルを管理しているのが"series"というファイルです。
```
cat debian/patches/series 
debian-changes-1.3-7
hardening
cross-build-dh_auto_build
Fix-FBTFS-for-time_t-transition
Fix-FBTFS-for-mkconf
0008-Update-FSF-address.patch
0007-Fix-1114447-FTBFS.patch
0008-blhc-tells-me-need-more-hardening.patch
```
のようにパッチの管理をしている。

### gbp pq は何をするコマンドなのか?

Debian packageを管理するのに、"debian/patches/"にパッチを作る必要があります。`gbp pq import`で"patch-queue"ブランチを作り
"patch-queue/作業ブランチ"で"upstream"のソースを変更します。Gitの作法に則って、一行目にsummary、2行目は空けて、3行目
から詳細を書きます。書く内容は上記で説明した[DEP-3: Patch Tagging Guidelines](https://dep-team.pages.debian.net/deps/dep3/)
です。この内容はプログラムで自動的に作られません。作業者が必要な情報はコミットメッセージに含める。

`gbp pq export`でpatch-queueのコミット毎に、一行目をファイル名としてquilt形式のpatchがdebian/patches/に生成される。
patch-queueの内容を確認しなくていいなら(あなたがcommitするのを忘れっぽいなら:-))、`gbp pq export --commit`とするのもいいだろう。

そして、`--drop`オプションを付けている場合patch-queueブランチは削除される。

この状態で、`gbp buildpackage`を動かすと、buildする前にdebian/patches/の内容が適用されてビルドが動きます。

### gbp pq でよく使うコマンド

下記の場合は、一般的にupstreamのソースコードを変更する場合の話です。新しくupstreamがリリースをした場合は、gbpを使ってnew upstreamに対応するの部分を読んでください。

`gbp pq import --force --time-machine=10`

`--force`オプションは下記のように、patch-queueブランチが存在していても、現在のdebian/patches/の内容でpatch-queueブランチを上書きします。
> In case of import, import even if the patch-queue branch already exists and overwrite its content with debian/patches.


`--time-machine=数値`オプションは下記の意味になります。

> --time-machine=NUM
> When importing a patch queue fails, go back commit-by-commit on the current branch to check if the patch-queue applies there.
> Do this at most NUM times. This can be useful if the patch-queue doesn't apply to the current branch HEAD anymore,
> e.g. after importing a new upstream version.

patch-queueのインポートに失敗しすると、現在のブランチのコミット毎にコミットを遡ってpatch-queueが適用できるか確認します。
(コミットを)遡る回数を指定します。新しいupstream versionをインポートする時など、現在のブランチのHEADにもはやpatch-queueが適用できない時に有用です。

`gbp pq export --drop --commit`

`--[no-]drop`
> Whether to automatically drop (delete) the patch queue branch after a successful export

`gbp pq export`と一緒に使う場合、exportが成功した後にpatch-queueブランチは削除されます。指定しない場合はpatch-queueブランチを消しません。

`--commit`
> In case of export, commit debian/patches the changes to Git after exporting the patches.

`gbp pq export`で指定された場合、patchesをエキスポートした後にdebian/patchesへの変更を`git commit`します。

本来は、下記のようにする必要があります。
```
git add debian/patches
git commit
```
上記をする必要がなく、操作を楽にします。

## gbp dch --release

ちゃんと、patch-queueでコミットを作っていると、debian/changelogに一行目のサマリーを転写してchangelogを作るのに楽ができます。

## gbpを使ってnew upstramに対応する

- `debian/watch`を設定していたら、uscanで新しいリリース(tarballなど)を入手します
- 入手したtarballをgbp importします。ここでミスるとpristine-tarのdelta管理で失敗して、オリジナルのtarballの内容と違うじゃねーか。という困った状況になるのでよく調べてから実行する。`gbp import-orig --uscan`から調べるのがよい
- importが成功しました、次は既存のパッチ(debian/patches/が存在するものたち)が新しいリリースでも必要か？うまく適用できるか？などを確認します。
- ここで`gbp pq rebase`が活躍します
- 不必要なパッチは`gbp pq drop`し必要なパッチを残し、場合によっては書き換えてコミットして`debian/patches/`に書き出すパッチを整理していきます
- 作業が終了したら、`gbp pq export`して、コミットからパッチを生成します。
- debianディレクトリ配下のファイルを書き換える必要があれば、`--debian-branch=`で指定しているブランチで作業してコミットする。`debian/changelog`に関しては`gbp dch --release`コマンドを使えばコミットからchangelogの雛形を生成してくれる。
- ローカルでビルドやテストを通しているのは前提ですが、salsa.debian.orgのリポジトリにpushしてsalsa.debian.orgのCIチームが用意してくれているCIにかけます。
- 満足できる結果であれば、source only uploadをdputなどで行います

### `gbp pq rebase`の概要

1. アップストリームの新バージョンへの対応
アップストリームの新しいバージョンがリリースされた際に、既存のDebianパッチセットを新しいベースに適用し直す場合

例：libfoo 1.2.0用に作成したパッチをlibfoo 1.3.0に適用する

2. パッチの競合解決
アップストリームの変更とDebianパッチ間に競合が発生した場合の解決

競合を手動で解決しながらパッチを再適用する

3. パッチセットの整理
パッチの順序変更や統合が必要な場合

不要になったパッチの削除やパッチ間の依存関係調整


実例
```
gbp pq rebase
gbp:info: No pq branch found, importing patches
gbp:info: Trying to apply patches at '9e1b621a05637b260c349b766cc935ac53889bce'
gbp:info: 8 patches listed in 'debian/patches/series' imported on 'patch-queue/master'
gbp:info: Switching to 'patch-queue/master'
Current branch patch-queue/master is up to date.
```

`gbp pq rebase`コマンドを実行すると、patch-queue/作業ブランチに移される。ここで、gitコマンドや、エディタを使って意図したパッチを作ったり、落としたりします。
必要に応じて、`gbp pq export`コマンドでdebian/patches/にパッチを書き出すのもよいでしょう。書き出さないなら、`gbp pq switch`でブランチ移動するのもよいでしょう。

現時点では、`gbp pq switch`がgit switchに対する優位を理解してないので理解したら追記する。インタフェースの統一以外あるんだろうか。

`gbp pq rebase`は、git rebaseに似て、失敗した所で止まって、コードや設定を書き換えてコミットを作り、次のパッチにすすめるので問題の範囲を小さくしておける。
オプションも `--continue`, `--abort`, `--skip`などのgitと同じなので理解しやすい。問題が起きて、修正できたコミットが作れたら、`--continue`だし、不要なpatchだと判断できたら`--skip`だし、中断して他を調べてから戻りたいなら、`--abort`を指定することになる。

upstreamの変更に対してパッチの順番や、内容をhealthyに保ちたいときに使う。

#### 代替手段との比較

- `gbp pq apply`：単純にパッチを適用するのみ
- `gbp pq rebase`：競合解決やパッチの再編成が必要な場合

## 参考文献

### Debian

1. [Projects/DebSrc3.0 - Debian Wiki](https://wiki.debian.org/Projects/DebSrc3.0)

### gbp pq

1. [gbp-pq(1) — git-buildpackage — Debian trixie — Debian Manpages](https://manpages.debian.org/trixie/git-buildpackage/gbp-pq.1.en.html)
2. [Working with Patches: Building Debian Packages with git-buildpackage](https://honk.sigxcpu.org/projects/git-buildpackage/manual-html/gbp.patches.html)

### 日本語で読めるもの

- [東京エリア Debian 勉強会 Debian パッケージング道場 第 130 回 2015 年 9 月度](https://tokyodebian-team.pages.debian.net/pdf2015/debianmeetingresume201509-presentation.pdf)
    - pdfです。
- [2019-04-11 git-buildpackageを用いたdebパッケージ管理方法の紹介](https://blog.cybozu.io/entry/2019/04/11/110000)

- quilt
    - [quiltの使い方 #Linux - Qiita](https://qiita.com/htano/items/9440073e5a08c43d66ba)

## 謝辞


## さいごに

|       件名         |   日付   |
|:----               |:--------:|
|記事を書きはじめた日|2025-09-17|
|  記事を公開した日  |2025-09-17|
|  記事を変更した日  |2025-09-27|

上記は、この記事の鮮度を判断する一助のために書き手が載せたものです。

詳細な変更履歴は、
[GitHub - yabuki/friendly-potato: zenn-contents](https://github.com/yabuki/friendly-potato)
を参照してください。

記事に対するTypoの指摘などは、pull reqをしてもらえるとありがたいです。
受け入れるかどうかは、差分とPull reqの文章で判断いたします。

<!-- 文章の目的は何か -->
<!-- 読み手に何の情報を伝えるのか -->
<!-- 読んだひとにどういう行動をしてもらいたいのか -->
<!-- だれに向けての文章か -->
<!-- この文章の肝はどこか -->
<!-- 画像はrepoのtopにあるimagesに入れよ -->
