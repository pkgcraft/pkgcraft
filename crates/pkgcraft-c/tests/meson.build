dep_parse = executable('dep', f'dep_parse.c', dependencies : [pkgcraft])
test('nonversion', dep_parse, args : ['cat/pkg'], suite: 'dep',
  env : ['category=cat', 'package=pkg'])
test('version', dep_parse, args : ['=cat/pkg-1'], suite: 'dep',
  env : ['category=cat', 'package=pkg', 'version=1'])
test('revision', dep_parse, args : ['=cat/pkg-1-r2'], suite: 'dep',
  env : ['category=cat', 'package=pkg', 'version=1-r2', 'revision=2'])
test('slotted', dep_parse, args : ['cat/pkg:1'], suite: 'dep',
  env : ['category=cat', 'package=pkg', 'slot=1'])
test('subslot', dep_parse, args : ['cat/pkg:1/2'], suite: 'dep',
  env : ['category=cat', 'package=pkg', 'slot=1', 'subslot=2'])
test('slot_op', dep_parse, args : ['cat/pkg:*'], suite: 'dep',
  env : ['category=cat', 'package=pkg', 'slot_op=*'])
test('use_deps', dep_parse, args : ['cat/pkg[a,b,c]'], suite: 'dep',
  env : ['category=cat', 'package=pkg', 'use_deps=a,b,c'])
test('repo', dep_parse, args : ['cat/pkg::repo'], suite: 'dep',
  env : ['category=cat', 'package=pkg', 'repo=repo'])

dep_cmp = executable('dep_cmp', f'dep_cmp.c', dependencies : [pkgcraft])
test('less', dep_cmp, args : ['cat/apkg', 'cat/bpkg', '-1'], suite: 'dep')
test('less', dep_cmp, args : ['<cat/pkg-1', '>cat/pkg-1', '-1'], suite: 'dep')
test('equal', dep_cmp, args : ['cat/pkg', 'cat/pkg', '0'], suite: 'dep')
test('equal', dep_cmp, args : ['=cat/pkg-0', '=cat/pkg-0-r0', '0'], suite: 'dep')
test('greater', dep_cmp, args : ['>cat/pkg-1', '<cat/pkg-1', '1'], suite: 'dep')
test('greater', dep_cmp, args : ['=cat/pkg-2', '=cat/pkg-1', '1'], suite: 'dep')
