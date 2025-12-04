//go:build linux || darwin

package backend

import _ "embed"

//go:embed bin/extractor
var extractorBin []byte
