struct InitVolumeWork {
    vol: char,
    // void InitVolumeWork::run()
    // {
    //     Volume *volume = new Volume(m_vol);
    //     volume->BuildIndex();
    //     emit finished(volume);
    // }

    // InitVolumeWork::InitVolumeWork(const char vol)
    // {
    //     setAutoDelete(true);
    //     m_vol = vol;
    // }
}

struct FindWork {
    volume: Volume,
    filename: String,
    // FindWork::FindWork(Volume *volume, QString filename)
    // {
    //     setAutoDelete(true); // 停止现在正在进行的搜索操作
    //     m_volume = volume;
    //     m_filename = filename;
    // }

    // void FindWork::run(){
    //     vector<SearchResultFile>* result;
    //     result = m_volume->Find(m_filename);
    //     emit finished(m_filename, result);
    // }

    // void FindWork::stop(){
    //     m_volume->StopFind();
    // }
}

struct FileData {
    state: u8, // 0, created; 1, initing; 2, inited.
    vols: Vec<char>,
    volumes: Vec<Volume>,
    futures: Vec<Future<()>>,
    finding_name: String,
    finding_result: Vec<SearchResultFile>,
    waiting_finder: u8,
    waiting_init: u8,
}

impl FileData {
    pub fn new() {
        FileData {
            state: 0,
            vols: Vec::new(),
            volumes: Vec::new(),
            futures: Vec::new(),
            finding_name: String::new(),
            finding_result: Vec::new(),
            waiting_finder: 0,
            waiting_init: 0,
        }
    }

    pub fn initVolumes() {
        // TODO
    // bool FileData::initVolumes()
    // {
    //     state = 1;
    //     m_waitingInit = initValidVols();
    //     for (int i=0; i<m_vols.size(); ++i)
    //     {
    //         InitVolumeWork* work = new InitVolumeWork(m_vols.at(i));
    //         connect(work, &InitVolumeWork::finished, this, &FileData::onInitVolumeWorkFinished);
    //         QThreadPool::globalInstance()->start(work);
    //     }
    //     return true;
    // }
    }

    pub fn findFile(filename: String) {
        // TODO
    // void FileData::findFile(QString filename)
    // {
    //     if(state != 2 || m_findingName == filename) return; // 如果还未初始化 或 查找相同文件 则直接返回
    //     emit sgn_stopFind();

    //     m_findingName = filename;
    //     m_waitingFinder = m_volumes.length();
    //     delete m_findingResult;
    //     m_findingResult = new vector<SearchResultFile>();

    //     for(Volume* volume: m_volumes){
    //         FindWork* work = new FindWork(volume, m_findingName);
    //         connect(work, &FindWork::finished, this, &FileData::onFindWorkFinished);
    //         connect(this, &FileData::sgn_stopFind, work, &FindWork::stop);
    //         QThreadPool::globalInstance()->start(work);
    //     }
    // }
    }

    pub fn updateIndex() {
        // TODO
    // void FileData::updateIndex()
    // {
    //     for(Volume* volume: m_volumes){
    //         QFuture result = QtConcurrent::run([volume]{
    //             volume->UpdateIndex();
    //         });
    //     }
    // }
    }

    pub fn releaseIndex() {
        // TODO
    // void FileData::releaseIndex()
    // {
    //     for(Volume* volume: m_volumes){
    //         QFuture result = QtConcurrent::run([volume]{
    //             volume->ReleaseIndex();
    //         });
    //     }
    // }
    }

    fn initValidVols() -> u8 {
        // TODO
    // unsigned short FileData::initValidVols(){
    //     DWORD dwBitMask = GetLogicalDrives();
    //     m_vols.empty();
    //     char vol = 'a';
    //     while(dwBitMask != 0){
    //         if(dwBitMask & 0x1) if ( isNTFS(vol) ) m_vols.append(vol);;
    //         vol++;
    //         dwBitMask >>= 1;
    //     }
    //     return m_vols.length();
    // }
    }

    fn isNTFS(vol: char) -> bool {
        // TODO
    // bool FileData::isNTFS(char vol)
    // {
    //     char lpRootPathName[] = ("t:\\");
    //     lpRootPathName[0] = vol;
    //     char lpVolumeNameBuffer[MAX_PATH];
    //     DWORD lpVolumeSerialNumber;
    //     DWORD lpMaximumComponentLength;
    //     DWORD lpFileSystemFlags;
    //     char lpFileSystemNameBuffer[MAX_PATH];

    //     if ( GetVolumeInformationA(lpRootPathName,lpVolumeNameBuffer, MAX_PATH,
    //             &lpVolumeSerialNumber, &lpMaximumComponentLength, &lpFileSystemFlags,
    //             lpFileSystemNameBuffer, MAX_PATH)) {
    //         return !strcmp(lpFileSystemNameBuffer, "NTFS");
    //     }
    //     return false;
    // }
    }

    fn onInitVolumeWorkFinished(vol: char) {
        // TODO
    // void FileData::onInitVolumeWorkFinished(Volume *volume)
    // {
    //     m_volumes.append(volume);
    //     m_waitingInit--;
    //     if(m_waitingInit == 0) state = 2;
    // }
    }

    fn onFindWorkFinished(filename: String, result: Vec<SearchResultFile>) {
        // TODO
    // void FileData::onFindWorkFinished(QString filename, vector<SearchResultFile>* result)
    // {
    //     if(result == nullptr || filename != this->m_findingName) return;
    //     m_findingResult->insert(m_findingResult->end(), result->begin(), result->end());
    //     delete result;
    //     result = nullptr;
    //     if(--m_waitingFinder == 0){
    //         sort(m_findingResult->begin(), m_findingResult->end());
    //         emit sgn_updateSearchResult(filename, m_findingResult);
    //     }
    // }
    }
}