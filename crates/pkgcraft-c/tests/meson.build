atom = executable('atom', f'atom.c', dependencies : [pkgcraft])
test('nonversion', atom, args : ['cat/pkg'], suite: 'atom',
  env : ['category=cat', 'package=pkg'])
test('version', atom, args : ['=cat/pkg-1'], suite: 'atom',
  env : ['category=cat', 'package=pkg', 'version=1', 'revision=0'])
test('revision', atom, args : ['=cat/pkg-1-r2'], suite: 'atom',
  env : ['category=cat', 'package=pkg', 'version=1-r2', 'revision=2'])
test('slotted', atom, args : ['cat/pkg:1'], suite: 'atom',
  env : ['category=cat', 'package=pkg', 'slot=1'])
test('subslot', atom, args : ['cat/pkg:1/2'], suite: 'atom',
  env : ['category=cat', 'package=pkg', 'slot=1', 'subslot=2'])
test('slot_op', atom, args : ['cat/pkg:*'], suite: 'atom',
  env : ['category=cat', 'package=pkg', 'slot_op=*'])
test('use_deps', atom, args : ['cat/pkg[a,b,c]'], suite: 'atom',
  env : ['category=cat', 'package=pkg', 'use_deps=a,b,c'])
test('repo', atom, args : ['cat/pkg::repo'], suite: 'atom',
  env : ['category=cat', 'package=pkg', 'repo=repo'])

atom_cmp = executable('atom_cmp', f'atom_cmp.c', dependencies : [pkgcraft])
test('less', atom_cmp, args : ['cat/apkg', 'cat/bpkg', '-1'], suite: 'atom')
test('less', atom_cmp, args : ['<cat/pkg-1', '>cat/pkg-1', '-1'], suite: 'atom')
test('equal', atom_cmp, args : ['cat/pkg', 'cat/pkg', '0'], suite: 'atom')
test('equal', atom_cmp, args : ['=cat/pkg-0', '=cat/pkg-0-r0', '0'], suite: 'atom')
test('greater', atom_cmp, args : ['>cat/pkg-1', '<cat/pkg-1', '1'], suite: 'atom')
test('greater', atom_cmp, args : ['=cat/pkg-2', '=cat/pkg-1', '1'], suite: 'atom')
