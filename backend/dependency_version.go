package backend

type DependencyVersionStatus struct {
	Installed        bool   `json:"installed"`
	InstalledVersion string `json:"installed_version,omitempty"`
	LatestVersion    string `json:"latest_version,omitempty"`
}
