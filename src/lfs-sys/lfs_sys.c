#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#ifndef LFS_THREADSAFE
#define LFS_THREADSAFE
#endif	// LFS_THREADSAFE

#ifdef __GNUC__
#define UNUSED(x) UNUSED_##x __attribute__((__unused__))
#else
#define UNUSED(x) UNUSED_##x
#endif

#include "lfs_rambd.h"
#include "lfs_sys.h"

/**
 * @brief Joins two paths together with a '/' if needed
 *
 * @param dst Destination buffer for the joined path
 * @param part1 Left part of the path
 * @param part2 Right part of the path
 */
static void path_join(char *dst, const char *part1, const char *part2) {
	strcpy(dst, part1);
	if (part1[strlen(part1) - 1] != '/') strcat(dst, "/");
	strcat(dst, part2);
}

/** This is our internal context that wraps LFS' context internally */
static struct {
	lfs_sys_lock_t lock;
	lfs_sys_unlock_t unlock;
} CONTEXT = {0};

/** @brief wrapper for Rust's lock function to make it LFS compatible */
static int lfs_lock(const struct lfs_config *UNUSED(_)) {
	// this is a blocking lock on Rust's side so it always returns 0
	CONTEXT.lock();
	return 0;
}

/** @brief wrapper for Rust's unlock function to make it LFS compatible */
static int lfs_unlock(const struct lfs_config *UNUSED(_)) {
	CONTEXT.unlock();
	return 0;
}

/** @brief LFS filesystem */
lfs_t DISK = {0};
/** @brief LFS configuration */
struct lfs_config CFG = {0};
/** @brief LFS RAM block device */
struct lfs_rambd RAMBD = {0};

/**
 * @brief Returns true if the path exists (file or directory)
 *
 * @param path the path to check
 * @return true if the path exists
 * @return false if the path does not exist (LFS returns other than LFS_ERR_OK)
 */
static bool exists(const char *path) {
	struct lfs_info info = {0};
	int ret = lfs_stat(&DISK, path, &info);
	return ret != LFS_ERR_OK;
}

/**
 * @brief Mounts the LFS filesystem somewhere in memory with a static address
 * @note This function is not thread-safe and must be called exactly once.
 *
 * @param sizeMB the size of the filesystem in MB
 * @param lock disk access lock function (for MT operations)
 * @param unlock disk access unlock function (for MT operations)
 */
void lfs_sys_mount(size_t sizeMB, lfs_sys_lock_t lock,
		   lfs_sys_unlock_t unlock) {
	CONTEXT.lock = lock;
	CONTEXT.unlock = unlock;
	memset(&CFG, 0, sizeof(struct lfs_config));
	CFG.read_size = 1024;
	CFG.prog_size = 1024;
	CFG.block_size = 4096;
	CFG.block_count = (sizeMB * 1024 * 1024) / CFG.block_size;
	CFG.cache_size = 1024;
	CFG.lookahead_size = 1024;
	CFG.block_cycles = 500;
	CFG.context = &RAMBD;
	CFG.read = lfs_rambd_read;
	CFG.prog = lfs_rambd_prog;
	CFG.erase = lfs_rambd_erase;
	CFG.sync = lfs_rambd_sync;
	CFG.lock = lfs_lock;
	CFG.unlock = lfs_unlock;
	int err = LFS_ERR_OK;
	err = lfs_rambd_create(&CFG);
	assert(err == 0);
	err = lfs_format(&DISK, &CFG);
	assert(err == 0);
	err = lfs_mount(&DISK, &CFG);
	assert(err == 0);
}

/** @brief Returns the static address of the LFS filesystem */
lfs_t *lfs_sys_disk(void) { return &DISK; }

/** @brief Convenience to allocate a new zeroed lfs_file_t for Rust */
lfs_file_t *lfs_sys_file_new(void) {
	lfs_file_t *file = malloc(sizeof(lfs_file_t));
	memset(file, 0, sizeof(lfs_file_t));
	return file;
}

/** @brief Convenience to free an lfs_file_t for Rust */
void lfs_sys_file_free(lfs_file_t *file) { free(file); }

/** @brief Convenience to allocate a new zeroed lfs_dir_t for Rust */
lfs_dir_t *lfs_sys_dir_new(void) {
	lfs_dir_t *dir = malloc(sizeof(lfs_dir_t));
	memset(dir, 0, sizeof(lfs_dir_t));
	return dir;
}

/** @brief Convenience to free an lfs_dir_t for Rust */
void lfs_sys_dir_free(lfs_dir_t *dir) { free(dir); }

/**
 * @brief Returns the size of a directory in bytes (recursively)
 *
 * @param path the path to the directory
 * @return size_t the size of the directory in bytes
 */
static size_t sizeof_directory(const char *path) {
	size_t size = 0;
	lfs_dir_t dir = {0};
	struct lfs_info info = {0};
	int ret = lfs_dir_open(&DISK, &dir, path);
	if (ret != LFS_ERR_OK) return 0;
	while (lfs_dir_read(&DISK, &dir, &info) > 0) {
		if (info.type == LFS_TYPE_REG)
			size += info.size;
		else {
			char subpath[strlen(path) + strlen(info.name) + 2];
			memset(subpath, 0, sizeof(subpath));
			path_join(subpath, path, info.name);
			if (exists(subpath)) size += sizeof_directory(subpath);
		}
	}
	ret = lfs_dir_close(&DISK, &dir);
	assert(ret == LFS_ERR_OK);
	return size;
}

/**
 * @brief Returns the size of a file or directory in bytes (recursively)
 *
 * @param path the path to the file or directory
 * @param file_count the number of files in the directory
 * @param dir_count the number of directories in the directory
 * @return size_t the size of the file or directory in bytes
 */
static size_t sizeof_path(const char *path, size_t *file_count,
			  size_t *dir_count) {
	struct lfs_info info = {0};
	int ret = lfs_stat(&DISK, path, &info);
	if (ret != LFS_ERR_OK) return 0;
	if (info.type == LFS_TYPE_REG) {
		if (file_count) (*file_count)++;
		return info.size;
	} else {
		if (dir_count) (*dir_count)++;
		return sizeof_directory(path);
	}
}

/**
 * @brief Queries a path for all attributes associated with it and returns them
 *
 * @param path the path to query
 * @return lfs_sys_query_t* all attributes associated with the path
 */
lfs_sys_query_t *lfs_sys_attr_query_new(const char *path) {
	lfs_sys_query_t *attributes = malloc(sizeof(lfs_sys_query_t));
	memset(attributes, 0, sizeof(lfs_sys_query_t));
	char attr[sizeof(double)] = {0};
	lfs_getattr(&DISK, path, LFS_SYS_ATTR_TYPE_INO, &attr,
		    sizeof(attributes->ino));
	attributes->ino = *(int *)attr;
	lfs_getattr(&DISK, path, LFS_SYS_ATTR_TYPE_MODE, &attr,
		    sizeof(attributes->mode));
	attributes->mode = *(int *)attr;
	lfs_getattr(&DISK, path, LFS_SYS_ATTR_TYPE_USERID, &attr,
		    sizeof(attributes->uid));
	attributes->uid = *(int *)attr;
	lfs_getattr(&DISK, path, LFS_SYS_ATTR_TYPE_GROUPID, &attr,
		    sizeof(attributes->gid));
	attributes->gid = *(int *)attr;
	lfs_getattr(&DISK, path, LFS_SYS_ATTR_TYPE_BIRTHTIME, &attr,
		    sizeof(attributes->birthtime));
	attributes->birthtime = *(double *)attr;
	lfs_getattr(&DISK, path, LFS_SYS_ATTR_TYPE_ATIME, &attr,
		    sizeof(attributes->atime));
	attributes->atime = *(double *)attr;
	lfs_getattr(&DISK, path, LFS_SYS_ATTR_TYPE_MTIME, &attr,
		    sizeof(attributes->mtime));
	attributes->mtime = *(double *)attr;
	lfs_getattr(&DISK, path, LFS_SYS_ATTR_TYPE_CTIME, &attr,
		    sizeof(attributes->ctime));
	attributes->ctime = *(double *)attr;
	lfs_getattr(&DISK, path, LFS_SYS_ATTR_TYPE_LINK, &attr,
		    sizeof(attributes->link));
	attributes->link = *(bool *)attr;
	lfs_getattr(&DISK, path, LFS_SYS_ATTR_TYPE_NLINK, &attr,
		    sizeof(attributes->nlink));
	attributes->nlink = *(int *)attr;
	lfs_getattr(&DISK, path, LFS_SYS_ATTR_TYPE_SYMLINK, &attr,
		    sizeof(attributes->symlink));
	attributes->symlink = *(bool *)attr;
	attributes->size = sizeof_path(path, NULL, NULL);
	return attributes;
}

/**
 * @brief "Patches" the attributes of a path by going over what's different and
 * updating it in the filesystem.
 * @note you cannot change size. it asserts.
 *
 * @param path The path to patch and update the attributes of
 * @param attributes The attributes to patch
 */
void lfs_sys_attr_patch(const char *path, lfs_sys_query_t *attributes) {
	lfs_sys_query_t *current = lfs_sys_attr_query_new(path);
	if (current->ino != attributes->ino) {
		lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_INO,
			    &attributes->ino, sizeof(attributes->ino));
	}
	if (current->mode != attributes->mode) {
		lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_MODE,
			    &attributes->mode, sizeof(attributes->mode));
	}
	if (current->uid != attributes->uid) {
		lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_USERID,
			    &attributes->uid, sizeof(attributes->uid));
	}
	if (current->gid != attributes->gid) {
		lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_GROUPID,
			    &attributes->gid, sizeof(attributes->gid));
	}
	if (current->birthtime != attributes->birthtime) {
		lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_BIRTHTIME,
			    &attributes->birthtime,
			    sizeof(attributes->birthtime));
	}
	if (current->atime != attributes->atime) {
		lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_ATIME,
			    &attributes->atime, sizeof(attributes->atime));
	}
	if (current->mtime != attributes->mtime) {
		lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_MTIME,
			    &attributes->mtime, sizeof(attributes->mtime));
	}
	if (current->ctime != attributes->ctime) {
		lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_CTIME,
			    &attributes->ctime, sizeof(attributes->ctime));
	}
	if (current->link != attributes->link) {
		lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_LINK,
			    &attributes->link, sizeof(attributes->link));
	}
	if (current->nlink != attributes->nlink) {
		lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_NLINK,
			    &attributes->nlink, sizeof(attributes->nlink));
	}
	if (current->symlink != attributes->symlink) {
		lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_SYMLINK,
			    &attributes->symlink, sizeof(attributes->symlink));
	}
	lfs_sys_attr_query_free(current);
}

/** @brief Convenience function to free a attribute query on Rust side */
void lfs_sys_attr_query_free(lfs_sys_query_t *attributes) { free(attributes); }

/** @brief resets all attributes of a path. call this when a new path is created
 */
void lfs_sys_attr_reset(const char *path) {
	char zero[sizeof(double)] = {0};
	lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_INO, &zero, sizeof(int));
	lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_MODE, &zero, sizeof(int));
	lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_USERID, &zero, sizeof(int));
	lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_GROUPID, &zero, sizeof(int));
	lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_BIRTHTIME, &zero,
		    sizeof(double));
	lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_ATIME, &zero,
		    sizeof(double));
	lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_MTIME, &zero,
		    sizeof(double));
	lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_CTIME, &zero,
		    sizeof(double));
	lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_NLINK, &zero, sizeof(int));
	lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_LINK, &zero, sizeof(bool));
	lfs_setattr(&DISK, path, LFS_SYS_ATTR_TYPE_SYMLINK, &zero,
		    sizeof(bool));
}

/** @brief Convenience function to create a new info struct on Rust side */
lfs_info_t *lfs_sys_info_new(void) {
	lfs_info_t *info = malloc(sizeof(lfs_info_t));
	memset(info, 0, sizeof(lfs_info_t));
	return info;
}

/** @brief Convenience function to free a info struct on Rust side */
void lfs_sys_info_free(lfs_info_t *info) { free(info); }

/** @brief Convenience function to return configured block size of LFS disk */
int lfs_sys_get_block_size(void) { return DISK.cfg->block_size; }

/** @brief Convenience function to return configured block count of LFS disk */
int lfs_sys_get_block_count(void) { return DISK.cfg->block_count; }

/** @brief Convenience function to return configured block address of LFS disk
 */
double lfs_sys_get_device_address(void) {
	size_t addr = (size_t)DISK.cfg->context;
	double ret = (double)addr;
	return ret;
}

// internal function. used in statvfs to count free blocks
static int lfs_sys_statvfs_traverse(void *p, lfs_block_t UNUSED(_)) {
	lfs_sys_statvfs_t *stat = (lfs_sys_statvfs_t *)p;
	stat->bfree--;
	return 0;
}

/**
 * @brief Returns statistics about the filesystem.
 *
 * @return lfs_sys_statvfs_t* pointer to struct with statistics
 */
lfs_sys_statvfs_t *lfs_sys_statvfs_new(void) {
	lfs_sys_statvfs_t *stat = malloc(sizeof(lfs_sys_statvfs_t));
	memset(stat, 0, sizeof(lfs_sys_statvfs_t));
	// https://linux.die.net/man/2/statfs
	stat->type = 0x858458f6;  // RAMFS_MAGIC
	stat->bsize = DISK.cfg->block_size;
	stat->blocks = DISK.cfg->block_count;
	stat->bfree = stat->blocks;  // count down in traverse
	int res = lfs_fs_traverse(&DISK, lfs_sys_statvfs_traverse, stat);
	assert(res == LFS_ERR_OK);
	stat->bavail = stat->bfree;
	size_t dir_count = 0;
	size_t file_count = 0;
	sizeof_path("/", &file_count, &dir_count);
	stat->dirs = dir_count;
	stat->files = file_count;
	stat->ffree = DISK.cfg->file_max - stat->files;
	return stat;
}

/** @brief Frees a lfs_sys_statvfs_t struct in Rust side */
void lfs_sys_statvfs_free(lfs_sys_statvfs_t *stat) { free(stat); }
