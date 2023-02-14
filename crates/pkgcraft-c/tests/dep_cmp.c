#include <assert.h>
#include <stdio.h>
#include <stdlib.h>

#include <pkgcraft.h>

int main (int argc, char **argv) {
	Dep *d1, *d2;
	int value;

	if (argc != 4) {
		fprintf(stderr, "incorrect args\n");
		exit(1);
	}

	d1 = pkgcraft_dep_new(argv[1], NULL);
	d2 = pkgcraft_dep_new(argv[2], NULL);
	value = pkgcraft_dep_cmp(d1, d2);
	assert(value == atoi(argv[3]));

	pkgcraft_dep_free(d1);
	pkgcraft_dep_free(d2);

	return 0;
}
