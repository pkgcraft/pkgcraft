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
	char *atom, *expected, *concat_str;
	char *value;
	int enum_val;
	char **array_value;
	size_t length;
	Atom *a = NULL;
	const AtomVersion *v;

	if (argc == 2) {
		atom = argv[1];
	} else if (argc < 2) {
		fprintf(stderr, "missing required atom arg\n");
		exit(1);
	}

	a = pkgcraft_atom_new(atom, NULL);

	value = pkgcraft_atom_cpn(a);
	assert(strcmp(value, "cat/pkg") == 0);
	pkgcraft_str_free(value);
	value = pkgcraft_atom_category(a);
	assert(strcmp(value, getenv("category")) == 0);
	pkgcraft_str_free(value);
	value = pkgcraft_atom_package(a);
	assert(strcmp(value, getenv("package")) == 0);
	pkgcraft_str_free(value);

	expected = getenv("version");
	v = pkgcraft_atom_version(a);
	if (expected) {
		value = pkgcraft_version_str((AtomVersion *)v);
		assert(strcmp(value, expected) == 0);
		pkgcraft_str_free(value);
	} else {
		assert(v == NULL);
	}

	value = pkgcraft_atom_revision(a);
	expected = getenv("revision");
	if (expected) {
		assert(strcmp(value, expected) == 0);
		pkgcraft_str_free(value);
	} else {
		assert(value == NULL);
	}

	value = pkgcraft_atom_slot(a);
	expected = getenv("slot");
	if (expected) {
		assert(strcmp(value, expected) == 0);
		pkgcraft_str_free(value);
	} else {
		assert(value == NULL);
	}

	value = pkgcraft_atom_subslot(a);
	expected = getenv("subslot");
	if (expected) {
		assert(strcmp(value, expected) == 0);
		pkgcraft_str_free(value);
	} else {
		assert(value == NULL);
	}

	enum_val = pkgcraft_atom_slot_op(a);
	expected = getenv("slot_op");
	if (expected) {
		assert(enum_val == atoi(expected));
	} else {
		assert(enum_val == -1);
	}

	array_value = pkgcraft_atom_use_deps(a, &length);
	expected = getenv("use_deps");
	if (expected) {
		concat_str = join(array_value, ',', length);
		assert(strcmp(concat_str, expected) == 0);
		pkgcraft_str_array_free(array_value, length);
		free(concat_str);
	} else {
		assert(array_value == NULL);
	}

	value = pkgcraft_atom_repo(a);
	expected = getenv("repo");
	if (expected) {
		assert(strcmp(value, expected) == 0);
		pkgcraft_str_free(value);
	} else {
		assert(value == NULL);
	}

	pkgcraft_atom_free(a);

	return 0;
}
