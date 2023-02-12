#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <pkgcraft.h>

char *join(char **strs, char delim, size_t length) {
	char *res = calloc(128, sizeof(char));
	char sep[2] = { delim, '\0' };
	size_t i;

	for (i = 0; i < length; i++) {
		if (i > 0) {
			strcat(res, sep);
		}
		strcat(res, strs[i]);
	}

	return res;
}

int main (int argc, char **argv) {
	char *dep, *expected, *concat_str;
	char *value;
	int slot_op;
	char **use_deps;
	size_t length;
	PkgDep *a = NULL;
	const Version *v;

	if (argc == 2) {
		dep = argv[1];
	} else if (argc < 2) {
		fprintf(stderr, "missing required dep arg\n");
		exit(1);
	}

	a = pkgcraft_dep_new(dep, NULL);

	value = pkgcraft_dep_cpn(a);
	assert(strcmp(value, "cat/pkg") == 0);
	pkgcraft_str_free(value);
	value = pkgcraft_dep_category(a);
	assert(strcmp(value, getenv("category")) == 0);
	pkgcraft_str_free(value);
	value = pkgcraft_dep_package(a);
	assert(strcmp(value, getenv("package")) == 0);
	pkgcraft_str_free(value);

	expected = getenv("version");
	v = pkgcraft_dep_version(a);
	if (expected) {
		value = pkgcraft_version_str((Version *)v);
		assert(strcmp(value, expected) == 0);
		pkgcraft_str_free(value);
	} else {
		assert(v == NULL);
	}

	value = pkgcraft_dep_revision(a);
	expected = getenv("revision");
	if (expected) {
		assert(strcmp(value, expected) == 0);
		pkgcraft_str_free(value);
	} else {
		assert(value == NULL);
	}

	value = pkgcraft_dep_slot(a);
	expected = getenv("slot");
	if (expected) {
		assert(strcmp(value, expected) == 0);
		pkgcraft_str_free(value);
	} else {
		assert(value == NULL);
	}

	value = pkgcraft_dep_subslot(a);
	expected = getenv("subslot");
	if (expected) {
		assert(strcmp(value, expected) == 0);
		pkgcraft_str_free(value);
	} else {
		assert(value == NULL);
	}

	slot_op = pkgcraft_dep_slot_op(a);
	expected = getenv("slot_op");
	if (expected) {
		assert(slot_op == pkgcraft_dep_slot_op_from_str(expected));
	} else {
		assert(slot_op == 0);
	}

	use_deps = pkgcraft_dep_use_deps(a, &length);
	expected = getenv("use_deps");
	if (expected) {
		concat_str = join(use_deps, ',', length);
		assert(strcmp(concat_str, expected) == 0);
		pkgcraft_str_array_free(use_deps, length);
		free(concat_str);
	} else {
		assert(use_deps == NULL);
	}

	value = pkgcraft_dep_repo(a);
	expected = getenv("repo");
	if (expected) {
		assert(strcmp(value, expected) == 0);
		pkgcraft_str_free(value);
	} else {
		assert(value == NULL);
	}

	pkgcraft_dep_free(a);

	return 0;
}
