cimport pkgcraft

cdef class atom:
    cdef pkgcraft.Atom *_atom
    cdef public str category
    cdef public str package
    cdef public str version
    cdef public str slot
    cdef public str subslot
    cdef public str repo

    def __init__(self, str s):
        self._atom = pkgcraft.str_to_atom(s.encode())
        self.category = self._atom.category.decode()
        self.package = self._atom.package.decode()

        if self._atom.version is not NULL:
            self.version = self._atom.version.decode()
        else:
            self.version = None

        if self._atom.slot is not NULL:
            self.slot = self._atom.slot.decode()
        else:
            self.slot = None

        if self._atom.subslot is not NULL:
            self.subslot = self._atom.subslot.decode()
        else:
            self.subslot = None

        if self._atom.repo is not NULL:
            self.repo = self._atom.repo.decode()
        else:
            self.repo = None

    def __dealloc__(self):
        pkgcraft.atom_free(self._atom)
