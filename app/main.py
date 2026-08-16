import json
import httpx
import logging
from fastapi import FastAPI, Request, BackgroundTasks
from fastapi.responses import StreamingResponse, Response
from urllib.parse import urljoin

from app.config import settings
from app.core.vault import SessionVault
from app.core.sanitizer import Sanitizer
from app.middleware.session import SessionMiddleware
from app.utils.json_walker import walk_and_modify

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

app = FastAPI(title="PromptVeil", version="1.0.0")

# Initialize core components
vault = SessionVault(max_sessions=settings.vault.max_sessions, ttl_seconds=settings.vault.session_ttl_seconds)
sanitizer = Sanitizer(vault)

# Add Middleware
app.add_middleware(SessionMiddleware)

# HTTP client setup
client = httpx.AsyncClient(timeout=settings.upstream.timeout_seconds)

@app.on_event("shutdown")
async def shutdown_event():
    await client.aclose()

@app.get("/healthz")
def healthz():
    return {"status": "ok"}

async def handle_streaming_response(response: httpx.Response, session_id: str):
    """
    Handles Server-Sent Events (SSE) streaming.
    Yields desanitized chunks.
    """
    # Rolling buffer for handling tokens that span across chunks
    buffer = ""
    
    async for chunk in response.aiter_text():
        if not chunk:
            continue
            
        buffer += chunk
        
        # We need to yield complete SSE lines.
        # A simple approach: split by double newline, keep the last incomplete part in buffer.
        parts = buffer.split('\n\n')
        
        # The last part is either an incomplete line or empty string
        buffer = parts.pop()
        
        for part in parts:
            if part.startswith("data: "):
                # Extract the JSON payload from the SSE line
                data_str = part[6:]
                if data_str.strip() == "[DONE]":
                    yield f"data: {data_str}\n\n"
                    continue
                    
                try:
                    # Parse, desanitize, serialize
                    data_json = json.loads(data_str)
                    
                    def desanitize_str(s: str) -> str:
                        return sanitizer.desanitize_text(s, session_id)
                        
                    data_json = walk_and_modify(data_json, desanitize_str)
                    
                    new_data_str = json.dumps(data_json)
                    yield f"data: {new_data_str}\n\n"
                except json.JSONDecodeError:
                    # If it's not valid JSON, just desanitize the raw text
                    yield f"data: {sanitizer.desanitize_text(data_str, session_id)}\n\n"
            else:
                # Non-data SSE line (e.g., event: message)
                yield f"{part}\n\n"
                
    # Flush remaining buffer
    if buffer:
        yield sanitizer.desanitize_text(buffer, session_id)


@app.api_route("/{path:path}", methods=["GET", "POST", "PUT", "DELETE", "PATCH"])
async def proxy(request: Request, path: str):
    session_id = request.state.session_id
    
    # 1. Read Request Body
    body = await request.body()
    content_type = request.headers.get("Content-Type", "")
    
    # 2. Sanitize if JSON
    if "application/json" in content_type and body:
        try:
            req_json = json.loads(body)
            
            def sanitize_str(s: str) -> str:
                return sanitizer.sanitize_text(s, session_id)
                
            req_json = walk_and_modify(req_json, sanitize_str)
            body = json.dumps(req_json).encode("utf-8")
        except json.JSONDecodeError:
            logger.warning("Invalid JSON in request, skipping sanitization")
            
    # 3. Forward Request
    upstream_url = urljoin(settings.upstream.default_base_url, path)
    if request.url.query:
        upstream_url += f"?{request.url.query}"
        
    # Filter headers (remove host, content-length, etc.)
    excluded_headers = ["host", "content-length", "content-encoding"]
    headers = {k: v for k, v in request.headers.items() if k.lower() not in excluded_headers}
    headers["Content-Length"] = str(len(body))
    
    # Build request
    req = client.build_request(
        method=request.method,
        url=upstream_url,
        headers=headers,
        content=body
    )
    
    upstream_resp = await client.send(req, stream=True)
    
    # 4. Handle Response
    resp_content_type = upstream_resp.headers.get("Content-Type", "")
    
    if "text/event-stream" in resp_content_type:
        return StreamingResponse(
            handle_streaming_response(upstream_resp, session_id),
            status_code=upstream_resp.status_code,
            headers={
                k: v for k, v in upstream_resp.headers.items() 
                if k.lower() not in ["content-encoding", "content-length", "transfer-encoding"]
            }
        )
    else:
        # Batch response
        await upstream_resp.aread()
        resp_body = upstream_resp.content
        
        if "application/json" in resp_content_type and resp_body:
            try:
                resp_json = json.loads(resp_body)
                
                def desanitize_str(s: str) -> str:
                    return sanitizer.desanitize_text(s, session_id)
                    
                resp_json = walk_and_modify(resp_json, desanitize_str)
                resp_body = json.dumps(resp_json).encode("utf-8")
            except json.JSONDecodeError:
                # If valid JSON failed, just desanitize as raw string if possible
                try:
                    text_body = resp_body.decode('utf-8')
                    resp_body = sanitizer.desanitize_text(text_body, session_id).encode('utf-8')
                except Exception:
                    pass
        
        resp_headers = {
            k: v for k, v in upstream_resp.headers.items() 
            if k.lower() not in ["content-encoding", "content-length", "transfer-encoding"]
        }
        resp_headers["Content-Length"] = str(len(resp_body))
        
        return Response(
            content=resp_body,
            status_code=upstream_resp.status_code,
            headers=resp_headers
        )

if __name__ == "__main__":
    import uvicorn
    uvicorn.run("app.main:app", host=settings.server.host, port=settings.server.port, workers=settings.server.workers)
