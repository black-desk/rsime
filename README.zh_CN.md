<!--
SPDX-FileCopyrightText: 2025 Chen Linxuan <me@black-desk.cn>

SPDX-License-Identifier: MIT
-->

<!-- TODO: 更新项目名称 -->

# 模版

[![checks][badge-shields-io-checks]][actions]
[![commit activity][badge-shields-io-commit-activity]][commits]
[![contributors][badge-shields-io-contributors]][contributors]
[![release date][badge-shields-io-release-date]][releases]
![commits since release][badge-shields-io-commits-since-release]
[![codecov][badge-shields-io-codecov]][codecov]

<!-- TODO: 更新项目链接 -->

[badge-shields-io-checks]:
  https://img.shields.io/github/check-runs/black-desk/template/master

<!-- TODO: 更新项目链接 -->

[actions]: https://github.com/black-desk/template/actions

<!-- TODO: 更新项目链接 -->

[badge-shields-io-commit-activity]:
  https://img.shields.io/github/commit-activity/w/black-desk/template/master

<!-- TODO: 更新项目链接 -->

[commits]: https://github.com/black-desk/template/commits/master

<!-- TODO: 更新项目链接 -->

[badge-shields-io-contributors]:
  https://img.shields.io/github/contributors/black-desk/template

<!-- TODO: 更新项目链接 -->

[contributors]: https://github.com/black-desk/template/graphs/contributors

<!-- TODO: 更新项目链接 -->

[badge-shields-io-release-date]:
  https://img.shields.io/github/release-date/black-desk/template

<!-- TODO: 更新项目链接 -->

[releases]: https://github.com/black-desk/template/releases

<!-- TODO: 更新项目链接 -->

[badge-shields-io-commits-since-release]:
  https://img.shields.io/github/commits-since/black-desk/template/latest

<!-- TODO: 更新项目链接 -->

[badge-shields-io-codecov]:
  https://codecov.io/github/black-desk/template/graph/badge.svg?token=6TSVGQ4L9X
[codecov]: https://codecov.io/github/black-desk/template

[en](README.md) | zh_CN

<!-- TODO: 添加项目简介 -->

## 使用

<!-- TODO: 添加项目使用说明 -->

1. 使用gh，从模版创建仓库：

   ```bash
   gh repo create --public --template black-desk/template
   ```

2. 编辑项目文件，填充内容后移除所有的`TODO`。

3. 运行检查脚本，确定所有`TODO`均已被移除：

   ```bash
   ./scripts/ls-todo.sh
   ```

## 许可证

如无特殊说明，该项目的代码以GNU通用公共许可协议第三版或任何更新的版本开源，文档、配置文件以及开发维护过程中使用的脚本等以MIT许可证开源。

该项目遵守[REUSE规范]。

你可以使用[reuse-tool](https://github.com/fsfe/reuse-tool)生成这个项目的SPDX列表：

```bash
reuse spdx
```

[REUSE规范]: https://reuse.software/spec-3.3/
