pkgdep = executable('pkgdep', f'pkgdep.c', dependencies : [pkgcraft])
test('nonversion', pkgdep, args : ['cat/pkg'], suite: 'pkgdep',
  env : ['category=cat', 'package=pkg'])
test('version', pkgdep, args : ['=cat/pkg-1'], suite: 'pkgdep',
  env : ['category=cat', 'package=pkg', 'version=1'])
test('revision', pkgdep, args : ['=cat/pkg-1-r2'], suite: 'pkgdep',
  env : ['category=cat', 'package=pkg', 'version=1-r2', 'revision=2'])
test('slotted', pkgdep, args : ['cat/pkg:1'], suite: 'pkgdep',
  env : ['category=cat', 'package=pkg', 'slot=1'])
test('subslot', pkgdep, args : ['cat/pkg:1/2'], suite: 'pkgdep',
  env : ['category=cat', 'package=pkg', 'slot=1', 'subslot=2'])
test('slot_op', pkgdep, args : ['cat/pkg:*'], suite: 'pkgdep',
  env : ['category=cat', 'package=pkg', 'slot_op=*'])
test('use_deps', pkgdep, args : ['cat/pkg[a,b,c]'], suite: 'pkgdep',
  env : ['category=cat', 'package=pkg', 'use_deps=a,b,c'])
test('repo', pkgdep, args : ['cat/pkg::repo'], suite: 'pkgdep',
  env : ['category=cat', 'package=pkg', 'repo=repo'])

pkgdep_cmp = executable('pkgdep_cmp', f'pkgdep_cmp.c', dependencies : [pkgcraft])
test('less', pkgdep_cmp, args : ['cat/apkg', 'cat/bpkg', '-1'], suite: 'pkgdep')
test('less', pkgdep_cmp, args : ['<cat/pkg-1', '>cat/pkg-1', '-1'], suite: 'pkgdep')
test('equal', pkgdep_cmp, args : ['cat/pkg', 'cat/pkg', '0'], suite: 'pkgdep')
test('equal', pkgdep_cmp, args : ['=cat/pkg-0', '=cat/pkg-0-r0', '0'], suite: 'pkgdep')
test('greater', pkgdep_cmp, args : ['>cat/pkg-1', '<cat/pkg-1', '1'], suite: 'pkgdep')
test('greater', pkgdep_cmp, args : ['=cat/pkg-2', '=cat/pkg-1', '1'], suite: 'pkgdep')
