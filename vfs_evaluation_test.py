#!/usr/bin/env python3
"""
VFS Evaluation Test - Gestalt Virtual File System
Evalúa las capacidades del VFS y reporta alcance/limitaciones
"""
import asyncio
import sys
import os
from pathlib import Path
from datetime import datetime

# Agregar path de gestalt de forma portable
DEFAULT_GESTALT_PATH = Path(__file__).resolve().parent
GESTALT_PATH = Path(os.getenv("GESTALT_PATH", str(DEFAULT_GESTALT_PATH))).resolve()
if GESTALT_PATH.exists():
    sys.path.insert(0, str(GESTALT_PATH))

# Intentar importar el bridge de Gestalt
try:
    from gestalt_bridge import GestaltBridge
    GESTALT_AVAILABLE = True
except ImportError:
    GESTALT_AVAILABLE = False
    print("⚠️ Gestalt bridge no disponible, evaluando VFS directamente...")

# Resultados de la evaluación
EVAL_RESULTS = {
    "timestamp": datetime.now().isoformat(),
    "tests_passed": 0,
    "tests_failed": 0,
    "tests_skipped": 0,
    "capabilities": [],
    "limitations": [],
    "recommendations": []
}

def log_test(name, passed, details="", skipped=False):
    """Registrar resultado de test"""
    status = "✅ PASS" if passed else "❌ FAIL"
    print(f"{status}: {name}")
    if details:
        print(f"   → {details}")
    
    if skipped:
        EVAL_RESULTS["tests_skipped"] += 1
    elif passed:
        EVAL_RESULTS["tests_passed"] += 1
    else:
        EVAL_RESULTS["tests_failed"] += 1
    
    return passed

def analyze_vfs_capabilities():
    """Analiza capacidades del VFS basadas en el código"""
    print("\n" + "="*60)
    print("📊 ANÁLISIS DE CAPACIDADES VFS")
    print("="*60 + "\n")
    
    # Leer el código fuente del VFS
    vfs_path = GESTALT_PATH / "gestalt_timeline" / "src" / "services" / "vfs.rs"
    
    if not vfs_path.exists():
        log_test("VFS source exists", False, "Archivo no encontrado")
        return set()
    
    log_test("VFS source exists", True)
    
    with open(vfs_path, 'r', encoding='utf-8') as f:
        vfs_code = f.read()
    
    # Analizar features implementados
    features = {
        "read_to_string": "Lectura de archivos" in vfs_code or "read_to_string" in vfs_code,
        "write_string": "Escritura en memoria" in vfs_code or "write_string" in vfs_code,
        "create_dir_all": "Crear directorios" in vfs_code or "create_dir_all" in vfs_code,
        "flush": "Flush a disco" in vfs_code or "fn flush" in vfs_code,
        "locks": "Sistema de locks" in vfs_code or "acquire_lock" in vfs_code,
        "discard": "Descartar cambios" in vfs_code or "fn discard" in vfs_code,
        "versioning": "Control de versiones" in vfs_code or "version" in vfs_code,
        "pending_changes": "Ver cambios pendientes" in vfs_code or "pending_changes" in vfs_code,
    }
    
    print("📦 Features implementados:")
    implemented = set()
    for feature, available in features.items():
        status = "✅" if available else "❌"
        print(f"   {status} {feature}")
        
        if available:
            implemented.add(feature)
            EVAL_RESULTS["capabilities"].append(feature)
        else:
            EVAL_RESULTS["limitations"].append(f"Missing: {feature}")
    
    # Analizar limitaciones
    print("\n🔍 Limitaciones identificadas:")
    
    limitations_found = []
    
    # 1. No hay soporte para binary files
    if "read_to_string" in vfs_code and "read" not in vfs_code.lower().replace("read_to_string", ""):
        limitations_found.append("Solo texto (no binary)")
    else:
        limitations_found.append("Soporte binario limitado")
    
    # 2. No hay symlinks
    limitations_found.append("No soporte para symlinks")
    
    # 3. No hay permisos (chmod)
    limitations_found.append("No soporte para permisos Unix")
    
    # 4. No hay watch/monitoring
    limitations_found.append("No FileWatcher para cambios externos")
    
    # 5. No hay transacciones atómicas
    limitations_found.append("No transacciones atómicas (rollback)")
    
    for lim in limitations_found:
        print(f"   ⚠️ {lim}")
        EVAL_RESULTS["limitations"].append(lim)
    
    return implemented

def test_gestalt_integration():
    """Prueba integración con Gestalt Bridge"""
    print("\n" + "="*60)
    print("🔗 PRUEBA DE INTEGRACIÓN CON GESTALT")
    print("="*60 + "\n")
    
    if not GESTALT_AVAILABLE:
        log_test("Gestalt Bridge import", False, "No disponible", skipped=True)
        return False
    
    try:
        bridge = GestaltBridge()
        log_test("GestaltBridge initialization", True)
        
        # Probar status
        try:
            status = bridge.status()
            log_test("Gestalt status()", True, str(status)[:50])
        except Exception as e:
            log_test("Gestalt status()", False, str(e))
        
        # Probar MCP tools
        try:
            result = bridge.mcp_call("echo", {"msg": "test"})
            valid_result = bool(result) and isinstance(result, dict) and (
                "output" in result or "content" in result or "result" in result
            )
            log_test("MCP echo tool", valid_result, str(result)[:80])
        except Exception as e:
            log_test("MCP echo tool", False, str(e))
        
        return True
        
    except Exception as e:
        log_test("Gestalt Bridge", False, str(e))
        return False

def calculate_coverage(analyzed_features):
    """Calcula % de cobertura de features"""
    standard_features = {
        "read_to_string",
        "write_string",
        "create_dir_all",
        "flush",
        "locks",
        "discard",
        "versioning",
        "pending_changes",
        "read_bytes",
        "write_bytes",
        "file_watcher",
        "atomic_transactions",
        "symlinks",
        "permissions",
        "snapshots",
    }
    covered = analyzed_features.intersection(standard_features)
    coverage = (len(covered) / len(standard_features)) * 100
    
    print("\n" + "="*60)
    print(f"📈 COBERTURA: {coverage:.1f}%")
    print("="*60)
    
    return coverage

def generate_recommendations():
    """Genera recomendaciones de mejora"""
    print("\n💡 RECOMENDACIONES:")
    print("-" * 40)
    
    recs = [
        ("1. Alta", "Agregar soporte para archivos binarios (read/write bytes)"),
        ("2. Alta", "Implementar FileWatcher para sincronización externa"),
        ("3. Media", "Agregar transacciones atómicas con rollback"),
        ("4. Media", "Soporte para symlinks y permisos"),
        ("5. Baja", "Compresión de snapshots para estados grandes"),
        ("6. Baja", "API REST para monitoreo remoto del VFS"),
    ]
    
    for priority, rec in recs:
        print(f"   [{priority}] {rec}")
        EVAL_RESULTS["recommendations"].append(f"[{priority}] {rec}")

def main():
    print("🧪 VFS EVALUATION TEST - Gestalt")
    print("="*60)
    
    # 1. Analizar código fuente
    analyzed_features = analyze_vfs_capabilities()
    
    # 2. Probar integración
    test_gestalt_integration()
    
    # 3. Calcular cobertura
    coverage = calculate_coverage(analyzed_features)
    
    # 4. Recomendaciones
    generate_recommendations()
    
    # Resumen final
    print("\n" + "="*60)
    print("📋 RESUMEN")
    print("="*60)
    print(f"   Tests passed:  {EVAL_RESULTS['tests_passed']}")
    print(f"   Tests failed:  {EVAL_RESULTS['tests_failed']}")
    print(f"   Tests skipped: {EVAL_RESULTS['tests_skipped']}")
    print(f"   Coverage:      {coverage:.1f}%")
    print(f"   Capabilities:  {len(EVAL_RESULTS['capabilities'])}")
    print(f"   Limitations:   {len(EVAL_RESULTS['limitations'])}")
    
    # Guardar resultado
    output_file = GESTALT_PATH / "vfs_evaluation_report.json"
    import json
    with open(output_file, 'w', encoding='utf-8') as f:
        json.dump(EVAL_RESULTS, f, indent=2, ensure_ascii=False)
    
    print(f"\n💾 Reporte guardado: {output_file}")
    
    return coverage

if __name__ == "__main__":
    main()
