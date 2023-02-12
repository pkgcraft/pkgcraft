#include <assert.h>
#include <stdio.h>
#include <stdlib.h>

#include <pkgcraft.h>

int main (int argc, char **argv) {
	PkgDep *a1, *a2;
	int value;

	if (argc != 4) {
		fprintf(stderr, "incorrect pkgdep_cmp args\n");
		exit(1);
	}

	a1 = pkgcraft_pkgdep_new(argv[1], NULL);
	a2 = pkgcraft_pkgdep_new(argv[2], NULL);
	value = pkgcraft_pkgdep_cmp(a1, a2);
	assert(value == atoi(argv[3]));

	pkgcraft_pkgdep_free(a1);
	pkgcraft_pkgdep_free(a2);

	return 0;
}
