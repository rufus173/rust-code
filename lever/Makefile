CFLAGS=-g
lever : src/main.rs src/leverfile.rs src/database.rs src/terminal.rs src/terminal.o
	rustc $(CFLAGS) src/main.rs -Clink-arg=src/terminal.o -o $@ $(LIBS)
