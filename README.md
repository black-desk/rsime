<!--
SPDX-FileCopyrightText: 2025 Chen Linxuan <me@black-desk.cn>

SPDX-License-Identifier: MIT
-->

<!-- TODO: Update project name -->

# Template

[![checks][badge-shields-io-checks]][actions]
[![commit activity][badge-shields-io-commit-activity]][commits]
[![contributors][badge-shields-io-contributors]][contributors]
[![release date][badge-shields-io-release-date]][releases]
![commits since release][badge-shields-io-commits-since-release]
[![codecov][badge-shields-io-codecov]][codecov]

<!-- TODO: Update project links -->

[badge-shields-io-checks]:
  https://img.shields.io/github/check-runs/black-desk/template/master

<!-- TODO: Update project links -->

[actions]: https://github.com/black-desk/template/actions

<!-- TODO: Update project links -->

[badge-shields-io-commit-activity]:
  https://img.shields.io/github/commit-activity/w/black-desk/template/master

<!-- TODO: Update project links -->

[commits]: https://github.com/black-desk/template/commits/master

<!-- TODO: Update project links -->

[badge-shields-io-contributors]:
  https://img.shields.io/github/contributors/black-desk/template

<!-- TODO: Update project links -->

[contributors]: https://github.com/black-desk/template/graphs/contributors

<!-- TODO: Update project links -->

[badge-shields-io-release-date]:
  https://img.shields.io/github/release-date/black-desk/template

<!-- TODO: Update project links -->

[releases]: https://github.com/black-desk/template/releases

<!-- TODO: Update project links -->

[badge-shields-io-commits-since-release]:
  https://img.shields.io/github/commits-since/black-desk/template/latest

<!-- TODO: Update project links -->

[badge-shields-io-codecov]:
  https://codecov.io/github/black-desk/template/graph/badge.svg?token=6TSVGQ4L9X
[codecov]: https://codecov.io/github/black-desk/template

en | [zh_CN](README.zh_CN.md)

> [!WARNING]
>
> This English README is translated from the Chinese version using LLM and may
> contain errors.

<!-- TODO: Add project description -->

My personal project template

## Usage

<!-- TODO: Add project usage instructions -->

1. Use gh to create a repository from the template:

   ```bash
   gh repo create --public --template black-desk/template
   ```

2. Edit project files, fill in content, and remove all `TODO` comments.

3. Run the check script to ensure all `TODO` comments have been removed:

   ```bash
   ./scripts/ls-todo.sh
   ```

## License

Unless otherwise specified, the code of this project are open source under the
GNU General Public License version 3 or any later version, while documentation,
configuration files, and scripts used in the development and maintenance process
are open source under the MIT License.

This project complies with the [REUSE specification].

You can use [reuse-tool](https://github.com/fsfe/reuse-tool) to generate the
SPDX list for this project:

```bash
reuse spdx
```

[REUSE specification]: https://reuse.software/spec-3.3/
