#include <sys/ioctl.h>
#include <asm/termios.h>
#include <asm/termbits.h>
#include <unistd.h>

int ioctl_get_term_size(int *width, int *height){
	struct winsize term_size = {0};
	if (ioctl(STDOUT_FILENO,TIOCGWINSZ,&term_size) < 0) return -1;
	*width = term_size.ws_col;
	*height = term_size.ws_row;
	return 0;
}
