# Security Configuration

## OAuth Credentials

This project requires Google OAuth credentials for Gemini API access. **Never commit your actual credentials to the repository.**

### Setup Instructions

1. **Get OAuth Credentials:**
   - Go to [Google Cloud Console](https://console.cloud.google.com/)
   - Create or select a project
   - Enable the Vertex AI API
   - Go to "APIs & Services" > "Credentials"
   - Create OAuth 2.0 Client ID credentials
   - Download the credentials

2. **Configure Environment Variables:**
   - Copy `gestalt_timeline/.env.example` to `gestalt_timeline/.env`
   - Set your credentials:
     ```bash
     GOOGLE_OAUTH_CLIENT_ID=your_client_id_here.apps.googleusercontent.com
     GOOGLE_OAUTH_CLIENT_SECRET=your_client_secret_here
     ```

3. **Keep Credentials Secure:**
   - The `.env` file is already in `.gitignore` and will not be committed
   - Never share your credentials publicly
   - Rotate credentials if they are accidentally exposed

## Environment Variables

All sensitive configuration should be stored in environment variables or `.env` files, never hardcoded in source files.

See `gestalt_timeline/.env.example` for all available configuration options.
