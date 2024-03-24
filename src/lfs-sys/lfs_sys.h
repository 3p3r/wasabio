#ifndef LFS_SYS_H
#define LFS_SYS_H

#include "../lfs/lfs.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef enum lfs_sys_file_type {
	LFS_SYS_S_IFIFO = 0010000,
	LFS_SYS_S_IFREG = 0100000,
	LFS_SYS_S_IFDIR = 0040000,
	LFS_SYS_S_IFLNK = 0120000,
	LFS_SYS_S_IFMT = 0170000,
} lfs_sys_file_type_t;

typedef void (*lfs_sys_lock_t)(void);
typedef void (*lfs_sys_unlock_t)(void);

lfs_t *lfs_sys_disk(void);
void lfs_sys_mount(size_t, lfs_sys_lock_t, lfs_sys_unlock_t);
lfs_file_t *lfs_sys_file_new(void);
void lfs_sys_file_free(lfs_file_t *);
lfs_dir_t *lfs_sys_dir_new(void);
void lfs_sys_dir_free(lfs_dir_t *);
lfs_info_t *lfs_sys_info_new(void);
void lfs_sys_info_free(lfs_info_t *);
typedef enum {
	LFS_SYS_ATTR_TYPE_INO = 0,
	LFS_SYS_ATTR_TYPE_MODE = 1,
	LFS_SYS_ATTR_TYPE_USERID = 2,
	LFS_SYS_ATTR_TYPE_GROUPID = 3,
	LFS_SYS_ATTR_TYPE_BIRTHTIME = 4,
	LFS_SYS_ATTR_TYPE_ATIME = 5,
	LFS_SYS_ATTR_TYPE_MTIME = 6,
	LFS_SYS_ATTR_TYPE_CTIME = 7,
	LFS_SYS_ATTR_TYPE_NLINK = 8,
	LFS_SYS_ATTR_TYPE_LINK = 9,
	LFS_SYS_ATTR_TYPE_SYMLINK = 10,
} lfs_sys_attr_type_t;
typedef struct {
	int ino;
	int mode;
	int uid;
	int gid;
	double birthtime;
	double atime;
	double mtime;
	double ctime;
	int nlink;
	bool link;
	bool symlink;
	size_t size;
} lfs_sys_query_t;
lfs_sys_query_t *lfs_sys_attr_query_new(const char *);
void lfs_sys_attr_patch(const char *, lfs_sys_query_t *);
void lfs_sys_attr_query_free(lfs_sys_query_t *);
void lfs_sys_attr_reset(const char *);
lfs_info_t *lfs_sys_info_new(void);
void lfs_sys_info_free(lfs_info_t *);
int lfs_sys_get_block_size(void);
int lfs_sys_get_block_count(void);
double lfs_sys_get_device_address(void);
typedef struct {
	size_t type;
	size_t bsize;
	size_t blocks;
	size_t bfree;
	size_t bavail;
	size_t files;
	size_t ffree;
	size_t dirs;
} lfs_sys_statvfs_t;
lfs_sys_statvfs_t *lfs_sys_statvfs_new(void);
void lfs_sys_statvfs_free(lfs_sys_statvfs_t *);

#ifdef __cplusplus
}
#endif

#endif
