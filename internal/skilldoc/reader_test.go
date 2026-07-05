package skilldoc

import (
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestReaderListsAndReadsSkillFiles(t *testing.T) {
	root := newProjectTempDir(t)
	writeTestFile(t, root, "SKILL.md", "# Skill\n\n说明")
	writeTestFile(t, root, "references/workflow.md", "按步骤执行")
	writeTestFile(t, root, "scripts/prepare.js", "console.log('prepare')\n")

	reader := Reader{}
	files, err := reader.List(root)
	if err != nil {
		t.Fatalf("List() error = %v", err)
	}

	want := []FileInfo{
		{RelativePath: "SKILL.md", Name: "SKILL.md", Ext: "md", Size: int64(len("# Skill\n\n说明"))},
		{RelativePath: "references/workflow.md", Name: "workflow.md", Ext: "md", Size: int64(len("按步骤执行"))},
		{RelativePath: "scripts/prepare.js", Name: "prepare.js", Ext: "js", Size: int64(len("console.log('prepare')\n"))},
	}
	if !reflect.DeepEqual(files, want) {
		t.Fatalf("List() = %#v, want %#v", files, want)
	}

	doc, err := reader.Read(root, "references/workflow.md")
	if err != nil {
		t.Fatalf("Read() error = %v", err)
	}
	if doc.RelativePath != "references/workflow.md" {
		t.Fatalf("Document.RelativePath = %q, want %q", doc.RelativePath, "references/workflow.md")
	}
	if doc.Text != "按步骤执行" {
		t.Fatalf("Document.Text = %q, want %q", doc.Text, "按步骤执行")
	}
	if doc.Size != int64(len("按步骤执行")) {
		t.Fatalf("Document.Size = %d, want %d", doc.Size, len("按步骤执行"))
	}
	if !filepath.IsAbs(doc.AbsolutePath) {
		t.Fatalf("Document.AbsolutePath = %q, want absolute path", doc.AbsolutePath)
	}
}

func TestReaderNormalizesUppercaseExtension(t *testing.T) {
	root := newProjectTempDir(t)
	writeTestFile(t, root, "README.MD", "upper extension")

	files, err := (Reader{}).List(root)
	if err != nil {
		t.Fatalf("List() error = %v", err)
	}

	want := []FileInfo{
		{RelativePath: "README.MD", Name: "README.MD", Ext: "md", Size: int64(len("upper extension"))},
	}
	if !reflect.DeepEqual(files, want) {
		t.Fatalf("List() = %#v, want %#v", files, want)
	}
}

func TestReaderListSkipsUnreadableLargeAndIgnoredDirectories(t *testing.T) {
	root := newProjectTempDir(t)
	writeTestFile(t, root, "SKILL.md", "# Skill")
	writeTestFile(t, root, "references/guide.txt", "guide")
	writeTestFile(t, root, "assets/logo.png", "not really text")
	writeTestFile(t, root, "large.md", "123456789")
	writeTestFile(t, root, "node_modules/pkg/readme.md", "dependency")
	writeTestFile(t, root, ".git/config", "git config")
	writeTestFile(t, root, "dist/bundle.js", "bundle")

	files, err := (Reader{MaxBytes: 8}).List(root)
	if err != nil {
		t.Fatalf("List() error = %v", err)
	}

	got := relativePaths(files)
	want := []string{"SKILL.md", "references/guide.txt"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("List() relative paths = %#v, want %#v", got, want)
	}
}

func TestReaderListAppliesFileCountAndDepthLimits(t *testing.T) {
	root := newProjectTempDir(t)
	writeTestFile(t, root, "SKILL.md", "# Skill")
	writeTestFile(t, root, "a.md", "a")
	writeTestFile(t, root, "b.md", "b")
	writeTestFile(t, root, "one/two.md", "two")
	writeTestFile(t, root, "one/two/three.md", "three")

	files, err := (Reader{MaxFiles: 2}).List(root)
	if err != nil {
		t.Fatalf("List() with MaxFiles error = %v", err)
	}
	if len(files) != 2 {
		t.Fatalf("List() with MaxFiles returned %d files, want 2", len(files))
	}

	files, err = (Reader{MaxDepth: 2}).List(root)
	if err != nil {
		t.Fatalf("List() with MaxDepth error = %v", err)
	}
	if got := relativePaths(files); containsString(got, "one/two/three.md") {
		t.Fatalf("List() with MaxDepth should skip deep files, got %#v", got)
	}
	if got := relativePaths(files); !containsString(got, "one/two.md") {
		t.Fatalf("List() with MaxDepth should keep files at the limit, got %#v", got)
	}
}

func TestFileInfoLessIsIrreflexiveForSkillFile(t *testing.T) {
	files := []FileInfo{{RelativePath: "SKILL.md"}}
	if fileInfoLess(files, 0, 0) {
		t.Fatal("fileInfoLess(files, 0, 0) = true, want false")
	}
}

func TestReaderRejectsTraversalLargeAndBinaryFiles(t *testing.T) {
	root := newProjectTempDir(t)
	writeTestFile(t, root, "large.md", "12345")
	writeTestFile(t, root, "binary.dat", "abc\x00def")
	writeTestFile(t, root, "inside.md", "inside")

	reader := Reader{MaxBytes: 4}

	for _, relativePath := range []string{
		"../outside.md",
		"..\\outside.md",
		filepath.Join(root, "inside.md"),
		"/foo",
		"\\foo",
	} {
		if _, err := reader.Read(root, relativePath); !errors.Is(err, ErrPathOutside) {
			t.Fatalf("Read(%q) error = %v, want %v", relativePath, err, ErrPathOutside)
		}
	}

	if _, err := reader.Read(root, "large.md"); !errors.Is(err, ErrFileTooLarge) {
		t.Fatalf("Read() large file error = %v, want %v", err, ErrFileTooLarge)
	}

	if _, err := (Reader{}).Read(root, "binary.dat"); !errors.Is(err, ErrBinaryFile) {
		t.Fatalf("Read() binary file error = %v, want %v", err, ErrBinaryFile)
	}
}

func TestReaderRejectsSymlinkPointingOutsideRoot(t *testing.T) {
	root := newProjectTempDir(t)
	outsideRoot := newProjectTempDir(t)
	writeTestFile(t, outsideRoot, "outside.md", "outside text")

	linkPath := filepath.Join(root, "link.md")
	targetPath, err := filepath.Abs(filepath.Join(outsideRoot, "outside.md"))
	if err != nil {
		t.Fatalf("Abs() error = %v", err)
	}
	if err := os.Symlink(targetPath, linkPath); err != nil {
		t.Skipf("当前环境不支持创建 symlink，跳过符号链接逃逸测试: %v", err)
	}

	if _, err := (Reader{}).Read(root, "link.md"); !errors.Is(err, ErrPathOutside) {
		t.Fatalf("Read() symlink outside error = %v, want %v", err, ErrPathOutside)
	}
}

func TestReaderRejectsSymlinkDirectoryPointingOutsideRoot(t *testing.T) {
	root := newProjectTempDir(t)
	outsideRoot := newProjectTempDir(t)
	writeTestFile(t, outsideRoot, "outside.md", "outside text")

	linkPath := filepath.Join(root, "linkdir")
	targetPath, err := filepath.Abs(outsideRoot)
	if err != nil {
		t.Fatalf("Abs() error = %v", err)
	}
	if err := os.Symlink(targetPath, linkPath); err != nil {
		t.Skipf("当前环境不支持创建 symlink，跳过符号链接目录逃逸测试: %v", err)
	}

	if _, err := (Reader{}).Read(root, "linkdir/outside.md"); !errors.Is(err, ErrPathOutside) {
		t.Fatalf("Read() symlink directory outside error = %v, want %v", err, ErrPathOutside)
	}
}

func TestReaderUsesDefaultMaxBytesWhenUnset(t *testing.T) {
	root := newProjectTempDir(t)
	writeTestFile(t, root, "at-limit.md", strings.Repeat("a", 256*1024))
	writeTestFile(t, root, "over-limit.md", strings.Repeat("a", 256*1024+1))

	if _, err := (Reader{}).Read(root, "at-limit.md"); err != nil {
		t.Fatalf("Read() at default limit error = %v, want nil", err)
	}

	if _, err := (Reader{}).Read(root, "over-limit.md"); !errors.Is(err, ErrFileTooLarge) {
		t.Fatalf("Read() over default limit error = %v, want %v", err, ErrFileTooLarge)
	}
}

func newProjectTempDir(t *testing.T) string {
	t.Helper()

	base := filepath.Join("..", "..", ".tmp", "skilldoc-tests")
	if err := os.MkdirAll(base, 0o755); err != nil {
		t.Fatalf("MkdirAll() error = %v", err)
	}
	dir, err := os.MkdirTemp(base, t.Name()+"-")
	if err != nil {
		t.Fatalf("MkdirTemp() error = %v", err)
	}
	t.Cleanup(func() {
		if err := os.RemoveAll(dir); err != nil {
			t.Fatalf("RemoveAll() error = %v", err)
		}
	})
	return dir
}

func writeTestFile(t *testing.T, root, relativePath, text string) {
	t.Helper()

	path := filepath.Join(root, filepath.FromSlash(relativePath))
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("MkdirAll() error = %v", err)
	}
	if err := os.WriteFile(path, []byte(text), 0o644); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}
}

func relativePaths(files []FileInfo) []string {
	paths := make([]string, 0, len(files))
	for _, file := range files {
		paths = append(paths, file.RelativePath)
	}
	return paths
}

func containsString(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}
