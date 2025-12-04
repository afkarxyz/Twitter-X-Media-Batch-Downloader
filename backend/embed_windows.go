//go:build windows

package backend

import _ "embed"

//go:embed bin/extractor.exe
var extractorBin []byte
