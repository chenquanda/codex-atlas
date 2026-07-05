package main

import (
	"context"
	"fmt"
	"log"
	"os"

	"codex-atlas/internal/app"
	"codex-atlas/internal/config"
	"codex-atlas/internal/ui"
)

func main() {
	paths, err := config.DefaultPaths()
	if err != nil {
		log.Fatal(err)
	}
	manager := app.NewManager(paths.StorePath, paths.CachePath, paths.TranslationPath, paths.CodexHome)
	if len(os.Args) > 1 && os.Args[1] == "--smoke-scan" {
		if err := manager.Load(); err != nil {
			log.Fatal(err)
		}
		if err := manager.RefreshScan(context.Background()); err != nil {
			log.Fatal(err)
		}
		if err := manager.RefreshStats(); err != nil {
			log.Fatal(err)
		}
		fmt.Printf("codexHome=%s abilities=%d skills=%d\n", paths.CodexHome, len(manager.Scan.Abilities), len(manager.Abilities("skill", "", true)))
		return
	}
	if err := ui.Run(manager, paths.WorkDir); err != nil {
		log.Fatal(err)
	}
}
