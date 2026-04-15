import os
from fastapi import FastAPI, UploadFile, File, HTTPException
from fastapi.responses import JSONResponse
import uvicorn

app = FastAPI()
RECEIVED_DIR = os.path.join(os.path.dirname(__file__), "received")
os.makedirs(RECEIVED_DIR, exist_ok=True)


@app.post("/upload")
async def upload_audio(audio: UploadFile = File(...)):
    if not audio.filename:
        raise HTTPException(status_code=400, detail="No filename provided")

    save_path = os.path.join(RECEIVED_DIR, audio.filename)
    contents = await audio.read()

    with open(save_path, "wb") as f:
        f.write(contents)

    return JSONResponse({
        "status": "ok",
        "filename": audio.filename,
        "size_bytes": len(contents)
    })


@app.get("/health")
async def health():
    return {"status": "ok"}


if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=8080)
