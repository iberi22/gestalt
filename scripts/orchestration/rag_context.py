import os
from typing import List
from langchain_community.vectorstores import Chroma
from langchain_huggingface import HuggingFaceEmbeddings
from langchain_community.document_loaders import DirectoryLoader, TextLoader
from langchain_text_splitters import RecursiveCharacterTextSplitter
from langchain_core.documents import Document

class RAGContext:
    def __init__(self, persist_directory: str = None):
        # Use a local model as we don't have OpenAI keys
        self.embeddings = HuggingFaceEmbeddings(model_name="all-MiniLM-L6-v2")
        self.persist_directory = persist_directory
        if persist_directory:
            self.vectorstore = Chroma(
                persist_directory=self.persist_directory,
                embedding_function=self.embeddings
            )
        else:
            self.vectorstore = Chroma(
                embedding_function=self.embeddings
            )

    def index_project(self, project_path: str):
        """Indexes code, docs, and configs from the project path."""
        # Define loaders for different file types
        # For simplicity in this implementation, we focus on common text-based files
        loaders = [
            DirectoryLoader(project_path, glob="**/*.md", loader_cls=TextLoader),
            DirectoryLoader(project_path, glob="**/*.rs", loader_cls=TextLoader),
            DirectoryLoader(project_path, glob="**/*.py", loader_cls=TextLoader),
            DirectoryLoader(project_path, glob="**/*.toml", loader_cls=TextLoader),
        ]

        docs = []
        for loader in loaders:
            try:
                docs.extend(loader.load())
            except Exception as e:
                print(f"Error loading files: {e}")

        if not docs:
            print("No documents found to index.")
            return

        text_splitter = RecursiveCharacterTextSplitter(chunk_size=1000, chunk_overlap=200)
        splits = text_splitter.split_documents(docs)

        self.vectorstore.add_documents(splits)
        print(f"Indexed {len(splits)} document chunks.")

    def retrieve(self, query: str, k: int = 5) -> List[Document]:
        """Retrieves relevant context based on the query."""
        return self.vectorstore.similarity_search(query, k=k)

    def add_result(self, result: str, metadata: dict = None):
        """Adds a new result to the context for future retrieval."""
        doc = Document(page_content=result, metadata=metadata or {"source": "execution_result"})
        self.vectorstore.add_documents([doc])
