package backend

import (
	"errors"
	"fmt"
	"strings"

	"github.com/zalando/go-keyring"
)

const authTokenKeyringService = "TwitterXMediaBatchDownloader"

func normalizeAuthTokenSlot(slot string) (string, error) {
	switch strings.ToLower(strings.TrimSpace(slot)) {
	case "public":
		return "public", nil
	case "private":
		return "private", nil
	default:
		return "", fmt.Errorf("invalid auth token slot: %q", slot)
	}
}

func authTokenKeyringUser(slot string) string {
	return "auth-token-" + slot
}

func GetStoredAuthToken(slot string) (string, error) {
	normalizedSlot, err := normalizeAuthTokenSlot(slot)
	if err != nil {
		return "", err
	}

	token, err := keyring.Get(authTokenKeyringService, authTokenKeyringUser(normalizedSlot))
	if errors.Is(err, keyring.ErrNotFound) {
		return "", nil
	}
	if err != nil {
		return "", err
	}

	return token, nil
}

func SetStoredAuthToken(slot, token string) error {
	normalizedSlot, err := normalizeAuthTokenSlot(slot)
	if err != nil {
		return err
	}

	if strings.TrimSpace(token) == "" {
		return ClearStoredAuthToken(normalizedSlot)
	}

	return keyring.Set(authTokenKeyringService, authTokenKeyringUser(normalizedSlot), token)
}

func ClearStoredAuthToken(slot string) error {
	normalizedSlot, err := normalizeAuthTokenSlot(slot)
	if err != nil {
		return err
	}

	err = keyring.Delete(authTokenKeyringService, authTokenKeyringUser(normalizedSlot))
	if errors.Is(err, keyring.ErrNotFound) {
		return nil
	}

	return err
}
