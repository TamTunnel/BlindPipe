import uuid
from fastapi import Request
from starlette.middleware.base import BaseHTTPMiddleware
from starlette.responses import Response

class SessionMiddleware(BaseHTTPMiddleware):
    async def dispatch(self, request: Request, call_next):
        # Extract Session ID from header or generate a new one
        session_id = request.headers.get("X-Session-ID")
        if not session_id:
            # Fallback to authorization header if X-Session-ID is missing
            auth = request.headers.get("Authorization")
            if auth:
                # Use a hash of the auth token to group the session?
                # Actually, standard practice for zero-trust proxy: if no session ID, generate one.
                # Client must pass X-Session-ID to maintain state across requests.
                session_id = f"sess_{uuid.uuid4().hex}"
            else:
                session_id = f"sess_{uuid.uuid4().hex}"
                
        # Inject into request state so downstream handlers can access it
        request.state.session_id = session_id
        
        response = await call_next(request)
        
        # Optionally echo back the session ID so client knows it
        response.headers["X-Session-ID"] = session_id
        return response
