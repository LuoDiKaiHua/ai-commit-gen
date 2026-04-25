请根据以下 Git 改动信息，生成规范的 commit 信息(输出纯文本格式, 无需使用任何格式)。

---

## 项目信息
- 项目名称: {{ project_name }}
- 当前分支: {{ branch }}

## 最近的 Commit 历史（了解项目风格）
```
{{recent_commits}}
```

## Staged 改动统计
```
{{staged_diff}}
```

## Staged 改动详情
```diff
{{staged_diff_detail}}
```

---

请使用中文生成 commit 信息，要求：
1. 遵循 Conventional Commits 规范（feature/fix/docs/style/refactor/test/chore 等）
2. 第一行为简短标题（不超过 72 字符）
3. 如果改动复杂，在标题后空一行，添加详细说明
4. 参考项目已有的 commit 风格
5. **只输出 commit 信息本身，不要有任何额外解释**
6. 使用英文标点符号替代中文标点符号，注意中英文间留空格
7. 第一行各段的含义: 提交类型、需求名称、改动概述

示例格式：
```
[feature](用户登录): 添加用户登录功能

- 实现 JWT token 验证
- 添加登录/登出接口
- 更新用户状态管理
```
"""